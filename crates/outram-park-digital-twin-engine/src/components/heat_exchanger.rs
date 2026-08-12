//! Schematic two-stream recuperator art.
//!
//! A heat exchanger is the most general piece of equipment on a plant
//! schematic: two single-phase streams, separated by a wall, one giving heat to
//! the other. Unlike a condenser — where the interesting thing is the phase
//! change — nothing about a recuperator is interesting *except the arrangement*.
//! Which way the two streams run relative to each other decides how much of the
//! available temperature difference the exchanger can actually use, and it is
//! the one thing a reader must be able to see at a glance.
//!
//! So the artwork draws, in order:
//!
//! - the **body**, either a shell with a tube bundle or a plate pack between two
//!   end frames, per [`HeatExchangerConstruction`];
//! - the **two streams**, each graded along **its own path** from its inlet to
//!   its outlet, so a stream visibly cools (or heats) as it crosses;
//! - **flow arrows on both streams**, drawn from each stream's real inlet to its
//!   real outlet — in [`HeatExchangerKind::CounterFlow`] they point in opposite
//!   directions, and that opposition is drawn, not merely captioned;
//! - the **nozzles / ports** at the ends the streams really enter and leave
//!   from, which move when the arrangement changes;
//! - the **terminal approaches** at each end, bracketed between the two streams
//!   and labelled with the temperature difference there;
//! - a **temperature-profile strip** under the body, plotting both streams
//!   against length, which is where the two profiles converging toward each
//!   other — or failing to — is unmistakable.
//!
//! # Why the arrangement is the whole point
//!
//! In parallel flow both streams enter at the same end, so the cold stream is
//! chasing a target that is running away from it: the two profiles converge
//! toward a common temperature and **the cold outlet can never reach, let alone
//! exceed, the hot outlet**. In counter-flow the cold stream leaves at the end
//! where the hot stream *arrives*, so it is exchanging against the hottest fluid
//! in the machine at exactly the point it is hottest itself, and the cold outlet
//! **can** come out above the hot outlet. That is a *temperature cross*, and it
//! is the single most useful fact about flow arrangement.
//!
//! Both statements are checkable rather than decorative:
//! [`HeatExchangerKind::permits_temperature_cross`] states which arrangement can
//! do it, and [`approach_verdict`] decides, from the four temperatures the
//! caller supplied, whether those numbers are consistent with the arrangement
//! being drawn. A parallel-flow exchanger handed a crossed pair of outlets is
//! drawn with an explicit "impossible for this arrangement" tag rather than
//! quietly rendered as though it were fine.
//!
//! **That check is a sign convention, not a model.** It compares the two
//! terminal approaches the caller's own numbers imply; it computes no duty, no
//! effectiveness and no outlet temperature, and it is not a rating method. The
//! rating algebra already exists in
//! [`outram_park_fork_dwsim_libs::heat_exchanger`] and belongs there, not in a
//! presentation crate — see this crate's "no new physics" rule.
//!
//! # Dispatch
//!
//! [`HeatExchangerKind`], [`HeatExchangerConstruction`] and
//! [`HeatExchangerVisualState`] are enums, not trait objects, per the
//! workspace's mandatory "no trait objects" Rust design rule: all three sets are
//! closed and known at compile time, so adding a member is a variant and the
//! compiler then points at every match that needs handling.
//!
//! Two axes are separated deliberately. **Arrangement** ([`HeatExchangerKind`])
//! is thermodynamics and changes where the streams enter and which way they run.
//! **Construction** ([`HeatExchangerConstruction`]) is mechanical and changes
//! what the inside of the body looks like. They are independent — every
//! construction can be plumbed either way round — so folding them into one enum
//! would have produced a variant list that lies about the geometry.
//!
//! # What is drawn from real state, and what is left neutral
//!
//! This is the honesty rule the whole widget library is built on, and this
//! component is a more interesting case than [`crate::components::condenser`],
//! because [`tampines::components::HeatExchanger`] is **not** state-free.
//!
//! It stores three things, all of them real:
//!
//! | Field | Drawn? |
//! |---|---|
//! | `arrangement` (co- / counter-current) | **yes** — it picks [`HeatExchangerKind`], so the neutral card still shows its true flow directions |
//! | `area` (heat-transfer area) | **yes**, as a label |
//! | `overall_coefficient` (`U`) | **yes**, as a label |
//!
//! and nothing else. Its `calculate` returns
//! `TampinesError::NotYetImplemented`, so there are **no temperatures behind
//! it**. The physics-backed path ([`HeatExchangerVisual::new`], whose signature
//! is preserved) therefore draws the complete machine with its real flow
//! directions, real arrows, real nozzle positions and its area and `U`
//! labelled — and paints **no temperature colour anywhere**, draws no approach
//! values and no profile. Every fluid region is neutral grey.
//!
//! The one thing that path picks by itself is the **construction**, which
//! defaults to [`HeatExchangerConstruction::ShellAndTube`]: the component does
//! not say whether it is a shell-and-tube or a plate unit, and something has to
//! be drawn. That is a drawing convention, stated here and changeable with
//! [`HeatExchangerVisual::with_construction`] — the same status as
//! [`crate::components::CondenserVisual::new`] defaulting to a two-pass
//! waterbox. It is not a claim about the caller's equipment.
//!
//! The state-driven path is [`HeatExchangerVisual::from_scalars`], the same
//! contract as [`crate::components::PipeVisual::from_scalars`]: the caller
//! passes **real state from its own model** — both streams' inlet and outlet
//! temperatures, and optionally the duty. That is a narrower interface, not a
//! fabricated one, and it is not a stub. The duty in particular is an
//! [`Option`], so a caller that has temperatures but no measured duty gets no
//! duty label rather than a plausible-looking number.
//!
//! # Colour
//!
//! Both streams are graded by the shared
//! [`crate::components::temperature_colour`] map, so a heat exchanger reads
//! identically to every other widget in this library: blue at the cold end of
//! the display range, neutral white at its **midpoint**, red at the hot end.
//! There is no second colour axis here — unlike a condenser or a turbine, a
//! recuperator carries no quality, because both sides stay single-phase.
//!
//! The gradient along each stream is a **display interpolation** between the two
//! temperatures the caller supplied (see [`lerp_temperature`]). A real profile
//! along a recuperator is exponential in position, not linear; computing it is
//! rating work and belongs in `tampines`, not here. What the artwork claims is
//! only what it is given: the endpoints are exact, and the path between them is
//! drawn straight.
//!
//! # What this is not
//!
//! **Offline demonstration artwork, not a validated model and not a design
//! drawing.** Every proportion here — including
//! [`SHELL_AND_TUBE_ASPECT_RATIO`] and [`PLATE_FRAME_ASPECT_RATIO`] — is chosen
//! by eye for legibility on screen and is dimensioned from no design whatsoever.
//! Nothing in this module may be cited or re-used as heat-exchanger design data.
//! Per `RESPONSIBLE_USE.md` this is for education, research and V&V only — not
//! for facility operation, reactor control, or safety-critical decisions.

use crate::components::temperature_colour;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense};
use egui::{Stroke, StrokeKind, Ui, Vec2, Widget};
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
use tampines::components::HeatExchanger;
use uom::si::area::square_meter;
use uom::si::f64::{Area, HeatTransfer, Power, TemperatureInterval, ThermodynamicTemperature};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::kilowatt;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;

// ── Envelope proportions ────────────────────────────────────────────────────

/// Width-to-height ratio a shell-and-tube exchanger is drawn at, dimensionless.
///
/// **Chosen by eye, not taken from a design.** A shell-and-tube unit is long and
/// slim — the tubes have to be long enough to transfer the duty, and the shell
/// is only as tall as the bundle plus its baffle clearance — and 2.05 : 1 is
/// enough to read that way while still leaving a band under the body for the
/// temperature-profile strip. See the module's "What this is not" section.
pub const SHELL_AND_TUBE_ASPECT_RATIO: f32 = 2.05;

/// Width-to-height ratio a plate-and-frame exchanger is drawn at,
/// dimensionless.
///
/// **Chosen by eye, not taken from a design.** A plate pack seen edge-on is much
/// squatter than a shell — a great many thin channels stacked between two heavy
/// end frames — so 1.35 : 1 reads as a different machine at a glance in a
/// gallery, which is the point of drawing the second construction at all.
pub const PLATE_FRAME_ASPECT_RATIO: f32 = 1.35;

/// Number of tube rows drawn in a shell-and-tube bundle.
const TUBE_ROWS: usize = 4;

/// Number of flow channels drawn in a plate pack.
///
/// Even, so the pack has as many hot channels as cold ones and both outermost
/// channels sit against an end plate.
const PLATE_CHANNELS: usize = 8;

// ── Arrangement ─────────────────────────────────────────────────────────────

/// Which way the two streams run relative to each other.
///
/// This is the thermodynamically significant axis and the reason this widget
/// exists in more than one form. It maps one-to-one onto
/// [`FlowArrangement`], the enum
/// [`tampines::components::HeatExchanger`] actually stores, so the
/// physics-backed path draws the arrangement the component really holds rather
/// than a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatExchangerKind {
    /// Counter-flow: the streams enter at **opposite ends** and run in opposite
    /// directions.
    ///
    /// The cold stream leaves at the end the hot stream arrives at, so it does
    /// its last exchanging against the hottest fluid in the machine. This is
    /// always at least as effective as parallel flow for the same `U`, `A` and
    /// heat-capacity rates, and it is the only single-pass arrangement that can
    /// produce a **temperature cross** — a cold outlet above the hot outlet.
    CounterFlow,
    /// Parallel (co-current) flow: the streams enter at the **same end** and run
    /// in the same direction.
    ///
    /// The temperature difference is largest at the inlet and decays along the
    /// length as the two profiles converge toward a common value, so the cold
    /// outlet can only ever approach the hot outlet from below and never pass
    /// it. Used where that is a feature — it limits the wall temperature at the
    /// inlet and bounds how far a temperature-sensitive cold stream can be
    /// heated.
    ParallelFlow,
}

impl HeatExchangerKind {
    /// Every arrangement, in the order a gallery should show them.
    pub const ALL: &'static [Self] = &[Self::CounterFlow, Self::ParallelFlow];

    /// The arrangement [`tampines::components::HeatExchanger`] stores, drawn as
    /// this kind.
    pub fn from_arrangement(arrangement: FlowArrangement) -> Self {
        match arrangement {
            FlowArrangement::CounterCurrent => Self::CounterFlow,
            FlowArrangement::CoCurrent => Self::ParallelFlow,
        }
    }

    /// This kind as the [`FlowArrangement`] the physics libraries take, so a
    /// caller can hand the studio's selection straight to a rating routine.
    pub fn arrangement(self) -> FlowArrangement {
        match self {
            Self::CounterFlow => FlowArrangement::CounterCurrent,
            Self::ParallelFlow => FlowArrangement::CoCurrent,
        }
    }

    /// Short display name, for a picker or a card caption.
    pub fn label(self) -> &'static str {
        match self {
            Self::CounterFlow => "Counter-flow",
            Self::ParallelFlow => "Parallel flow",
        }
    }

    /// What the arrangement does thermodynamically, in words.
    pub fn description(self) -> &'static str {
        match self {
            Self::CounterFlow => "streams enter at opposite ends and run against each other",
            Self::ParallelFlow => "streams enter at the same end and run together",
        }
    }

    /// Where the cold stream enters and leaves, relative to the hot stream —
    /// the one fact that explains every geometric difference between the kinds.
    pub fn cold_stream_path(self) -> &'static str {
        match self {
            Self::CounterFlow => "in at the hot outlet end, out at the hot inlet end",
            Self::ParallelFlow => "in and out at the same ends as the hot stream",
        }
    }

    /// Whether this arrangement can put the **cold outlet above the hot
    /// outlet** — a temperature cross.
    ///
    /// `true` for [`Self::CounterFlow`] and `false` for [`Self::ParallelFlow`].
    /// In parallel flow both streams start at the same end, so their profiles
    /// can only converge; the cold stream reaching the hot stream's outlet
    /// temperature is the limit, and passing it would need heat to flow from
    /// cold to hot at that end.
    pub fn permits_temperature_cross(self) -> bool {
        matches!(self, Self::CounterFlow)
    }

    /// Direction the cold stream is drawn in, as `+1.0` (left to right, the
    /// direction the hot stream always runs) or `-1.0` (right to left).
    ///
    /// The hot stream is always drawn left to right, so this single number is
    /// what makes counter-flow *visible* rather than merely captioned.
    pub fn cold_stream_direction(self) -> f32 {
        match self {
            Self::CounterFlow => -1.0,
            Self::ParallelFlow => 1.0,
        }
    }
}

// ── Construction ────────────────────────────────────────────────────────────

/// What the inside of the body looks like — the mechanical axis, independent of
/// [`HeatExchangerKind`].
///
/// Both constructions can be plumbed either way round, which is why this is a
/// separate enum rather than more variants on the arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatExchangerConstruction {
    /// Shell-and-tube: a bundle of tubes carrying the **hot** stream, inside a
    /// shell carrying the **cold** stream over segmental baffles.
    ///
    /// The workhorse of process and power plant: tolerant of high pressure and
    /// large temperature difference, mechanically cleanable, and easy to build
    /// large. Drawn long and slim
    /// ([`SHELL_AND_TUBE_ASPECT_RATIO`]).
    ShellAndTube,
    /// Plate-and-frame: a stack of thin pressed plates clamped between two end
    /// frames, with the two streams in **alternating channels**.
    ///
    /// Much more surface per unit volume than a shell-and-tube unit and capable
    /// of a very close approach, at the price of gasket-limited pressure and
    /// temperature. Drawn squat ([`PLATE_FRAME_ASPECT_RATIO`]), and the
    /// alternating channels make a counter-flow arrangement especially legible:
    /// adjacent channels carry arrows pointing opposite ways.
    PlateFrame,
}

impl HeatExchangerConstruction {
    /// Every construction, in the order a gallery should show them.
    pub const ALL: &'static [Self] = &[Self::ShellAndTube, Self::PlateFrame];

    /// Short display name, for a picker or a card caption.
    pub fn label(self) -> &'static str {
        match self {
            Self::ShellAndTube => "Shell and tube",
            Self::PlateFrame => "Plate and frame",
        }
    }

    /// Where this construction is normally used, in words.
    pub fn description(self) -> &'static str {
        match self {
            Self::ShellAndTube => "high pressure and large duty; cleanable, easy to build large",
            Self::PlateFrame => "compact, very close approach; gasket-limited p and T",
        }
    }

    /// Which stream is drawn inside the inner passages, in words — the tubes for
    /// a shell-and-tube unit, the alternating channels for a plate pack.
    pub fn hot_stream_location(self) -> &'static str {
        match self {
            Self::ShellAndTube => "hot in the tubes, cold in the shell",
            Self::PlateFrame => "hot and cold in alternating channels",
        }
    }

    /// Width-to-height ratio the artwork is drawn at, dimensionless.
    pub fn native_aspect_ratio(self) -> f32 {
        match self {
            Self::ShellAndTube => SHELL_AND_TUBE_ASPECT_RATIO,
            Self::PlateFrame => PLATE_FRAME_ASPECT_RATIO,
        }
    }

    /// The largest sub-rectangle of `available` carrying this construction's
    /// [`Self::native_aspect_ratio`], centred within it.
    ///
    /// Same letterbox contract as the condensers, steam generators and reactor
    /// vessels: the artwork keeps its proportions at any box size rather than
    /// stretching to fill its box, so a shell-and-tube unit stays long and slim
    /// in a square card. A degenerate box (zero or negative extent, as egui
    /// layout can transiently produce) is returned unchanged rather than
    /// producing NaN geometry.
    pub fn fit_native_aspect(self, available: Rect) -> Rect {
        let (w, h) = (available.width(), available.height());
        if w <= 0.0 || h <= 0.0 {
            return available;
        }
        let ratio = self.native_aspect_ratio();
        let (fw, fh) = if w / h > ratio {
            (h * ratio, h)
        } else {
            (w, w / ratio)
        };
        Rect::from_center_size(available.center(), Vec2::new(fw, fh))
    }
}

// ── Stream geometry ─────────────────────────────────────────────────────────

/// How far along **its own path** each stream is, at drawn length position `s`.
///
/// Returns `(hot, cold)`, both dimensionless fractions in `[0, 1]` where `0.0`
/// is that stream's own inlet and `1.0` its own outlet. `s` is a position across
/// the drawn body, `0.0` at the left edge and `1.0` at the right edge, clamped.
///
/// The hot stream is always drawn left to right, so its fraction is simply `s`.
/// The cold stream depends on the arrangement, and this is the single function
/// that encodes the difference:
///
/// - [`HeatExchangerKind::ParallelFlow`]: the cold stream also runs left to
///   right, so its fraction is `s` too — both streams are at their inlets at the
///   same end.
/// - [`HeatExchangerKind::CounterFlow`]: the cold stream runs right to left, so
///   its fraction is `1 - s` — at the left edge it is at its **outlet**, beside
///   the hot stream's inlet.
///
/// Colouring each stream between these fractions is what makes the two profiles
/// converge (or not) along the body.
pub fn path_fractions(kind: HeatExchangerKind, s: f32) -> (f32, f32) {
    let s = s.clamp(0.0, 1.0);
    match kind {
        HeatExchangerKind::ParallelFlow => (s, s),
        HeatExchangerKind::CounterFlow => (s, 1.0 - s),
    }
}

/// The two **terminal approaches**, as `(left_end, right_end)`.
///
/// A terminal approach is the temperature difference between the two streams at
/// one end of the exchanger — the driving force there. Both are returned as
/// signed [`TemperatureInterval`]s (`hot - cold` at that end), because the sign
/// is the whole diagnostic: a negative approach means heat would have to flow
/// from the cold stream to the hot one at that end, which cannot happen.
///
/// The pair returned is exactly the `(dt1, dt2)` that
/// [`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd`] forms for the
/// same arrangement, which is why the drawing's end-brackets can be read as the
/// two ends of the log-mean:
///
/// - [`HeatExchangerKind::CounterFlow`]: `(T_hot_in - T_cold_out,
///   T_hot_out - T_cold_in)`.
/// - [`HeatExchangerKind::ParallelFlow`]: `(T_hot_in - T_cold_in,
///   T_hot_out - T_cold_out)`.
///
/// No log-mean is taken here and no duty is computed — this is the geometry the
/// end brackets are drawn from, not a rating.
pub fn terminal_approaches(
    kind: HeatExchangerKind,
    hot_inlet_temp: ThermodynamicTemperature,
    hot_outlet_temp: ThermodynamicTemperature,
    cold_inlet_temp: ThermodynamicTemperature,
    cold_outlet_temp: ThermodynamicTemperature,
) -> (TemperatureInterval, TemperatureInterval) {
    let cold_at = |f: f32| lerp_temperature(cold_inlet_temp, cold_outlet_temp, f);
    let (_, cold_left) = path_fractions(kind, 0.0);
    let (_, cold_right) = path_fractions(kind, 1.0);
    (
        difference(hot_inlet_temp, cold_at(cold_left)),
        difference(hot_outlet_temp, cold_at(cold_right)),
    )
}

/// What the caller's four temperatures imply about the arrangement being drawn.
///
/// This is a **sign check on supplied numbers**, not a model: it computes no
/// duty, no effectiveness and no outlet temperature. Its only job is to let the
/// artwork say so when it is being asked to draw a combination that cannot
/// happen, instead of rendering it as though it were an ordinary operating
/// point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApproachVerdict {
    /// Both terminal approaches are positive and the cold outlet is at or below
    /// the hot outlet: an ordinary operating point for either arrangement.
    Feasible,
    /// Both terminal approaches are positive, but the **cold outlet is above the
    /// hot outlet** — a temperature cross.
    ///
    /// Reachable only in [`HeatExchangerKind::CounterFlow`]; see
    /// [`HeatExchangerKind::permits_temperature_cross`]. Worth drawing
    /// distinctly, because it is the headline capability of counter-flow and the
    /// reason it is chosen.
    TemperatureCross,
    /// A terminal approach is zero or negative: at one end the "cold" stream is
    /// at or above the "hot" stream, so heat would have to flow backwards.
    ///
    /// The combination cannot occur in the arrangement being drawn. For
    /// [`HeatExchangerKind::ParallelFlow`] this is what a crossed pair of
    /// outlets reduces to, which is why that kind can never reach
    /// [`Self::TemperatureCross`].
    Impossible,
}

/// Classify the caller's four temperatures against the arrangement — see
/// [`ApproachVerdict`].
///
/// Non-finite inputs give [`ApproachVerdict::Impossible`]: a NaN must be the
/// most visible outcome on screen rather than hiding behind a plausible label.
pub fn approach_verdict(
    kind: HeatExchangerKind,
    hot_inlet_temp: ThermodynamicTemperature,
    hot_outlet_temp: ThermodynamicTemperature,
    cold_inlet_temp: ThermodynamicTemperature,
    cold_outlet_temp: ThermodynamicTemperature,
) -> ApproachVerdict {
    let (left, right) = terminal_approaches(
        kind,
        hot_inlet_temp,
        hot_outlet_temp,
        cold_inlet_temp,
        cold_outlet_temp,
    );
    let (left, right) = (
        left.get::<kelvin_interval>(),
        right.get::<kelvin_interval>(),
    );
    if !left.is_finite() || !right.is_finite() {
        return ApproachVerdict::Impossible;
    }
    if left <= 0.0 || right <= 0.0 {
        return ApproachVerdict::Impossible;
    }
    if cold_outlet_temp.get::<kelvin>() > hot_outlet_temp.get::<kelvin>() {
        ApproachVerdict::TemperatureCross
    } else {
        ApproachVerdict::Feasible
    }
}

/// The temperature window the profile strip is plotted against, as
/// `(bottom, top)`.
///
/// Scaled to the **four terminal temperatures**, not to the display range: the
/// colour scale is usually set for a whole plant, and plotting a 20 K approach
/// against a 600 K scale would draw two flat lines on top of each other. A 12 %
/// margin is added at both ends so the extreme profiles do not sit on the strip
/// border, and the window is never narrower than **1 K**, so a degenerate state
/// (all four temperatures equal, which is what zero duty gives) still draws a
/// strip rather than collapsing to a line or dividing by zero.
///
/// Colour is unaffected — every colour in the artwork, profile strip included,
/// still comes from the caller's [`HeatExchangerDisplayRange`].
pub fn profile_temperature_bounds(
    scalars: &HeatExchangerScalars,
) -> (ThermodynamicTemperature, ThermodynamicTemperature) {
    let temps = [
        scalars.hot_inlet_temp.get::<kelvin>(),
        scalars.hot_outlet_temp.get::<kelvin>(),
        scalars.cold_inlet_temp.get::<kelvin>(),
        scalars.cold_outlet_temp.get::<kelvin>(),
    ];
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for t in temps {
        if t.is_finite() {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        // Nothing usable: a symmetric 1 K window about the freezing point, so
        // the strip still draws its axis and the profile lines land in it.
        return (
            ThermodynamicTemperature::new::<kelvin>(272.65),
            ThermodynamicTemperature::new::<kelvin>(273.65),
        );
    }
    // 12 % of the span at each end, and never narrower than 1 K.
    let window = ((hi - lo) * 1.24).max(1.0);
    let mid = 0.5 * (lo + hi);
    (
        ThermodynamicTemperature::new::<kelvin>(mid - 0.5 * window),
        ThermodynamicTemperature::new::<kelvin>(mid + 0.5 * window),
    )
}

// ── Small numeric helpers ───────────────────────────────────────────────────

/// Signed difference `hotter - colder`, in kelvin.
///
/// Built explicitly from the two kelvin readings rather than by subtracting the
/// absolute temperatures, matching `tampines::pebble_bed`'s convention for the
/// same conversion.
fn difference(
    hotter: ThermodynamicTemperature,
    colder: ThermodynamicTemperature,
) -> TemperatureInterval {
    TemperatureInterval::new::<kelvin_interval>(hotter.get::<kelvin>() - colder.get::<kelvin>())
}

/// Linear interpolation between two temperatures, in kelvin.
///
/// `t` is a dimensionless position along whatever path is being coloured,
/// clamped to `[0, 1]`. **This is a display interpolation, not physics**: the
/// real temperature profile along a recuperator is exponential in position, and
/// computing it is rating work that belongs in `tampines`, not in this
/// presentation crate. The endpoints are exact; the path between them is drawn
/// straight and is documented as such.
pub fn lerp_temperature(
    from: ThermodynamicTemperature,
    to: ThermodynamicTemperature,
    t: f32,
) -> ThermodynamicTemperature {
    let t = t.clamp(0.0, 1.0) as f64;
    ThermodynamicTemperature::new::<kelvin>(
        from.get::<kelvin>() * (1.0 - t) + to.get::<kelvin>() * t,
    )
}

/// The same colour at a reduced alpha, for fills that must not hide what is
/// drawn behind them.
fn translucent(colour: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
}

/// A corner radius in points, clamped to what egui's `u8` corner radii allow.
fn radius(points: f32) -> u8 {
    if !points.is_finite() {
        return 0;
    }
    points.round().clamp(0.0, 255.0) as u8
}

// ── Palette ─────────────────────────────────────────────────────────────────
//
// Matches `condenser`, `steam_generator` and `pump` exactly, so an exchanger, a
// condenser and a pump on the same schematic read as the same materials.

/// Pressure-boundary steel: the shell, the channel heads, the plate frames.
const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
/// Tubesheets and other heavy forgings, a shade lighter so they read as
/// separate parts rather than merging into the shell.
const FORGING: Color32 = Color32::from_rgb(126, 131, 140);
/// Vessel outline, drawn last so the silhouette reads on top.
const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
/// Internals that carry no interesting temperature: baffles, plate walls.
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
/// Unfilled interior, behind the coloured regions.
const VOID: Color32 = Color32::from_rgb(28, 30, 34);
/// Label text.
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);
/// Fluid colour used when no state is supplied. Neutral grey is the honest
/// drawing of "not known"; it is deliberately not a point on the temperature
/// scale. Same convention as `condenser::UNKNOWN_FLUID`.
const UNKNOWN_FLUID: Color32 = Color32::GRAY;
/// Amber, for the one annotation that is a warning rather than a measurement:
/// a set of temperatures that cannot occur in the arrangement being drawn.
const WARNING: Color32 = Color32::from_rgb(214, 148, 64);

// ── State ───────────────────────────────────────────────────────────────────

/// The temperature range the artwork's colours are graded against.
///
/// Both bounds are absolute thermodynamic temperatures (`uom`-typed, so the
/// compiler enforces the unit; kelvin internally, conventionally quoted in
/// degrees Celsius). The shared map is **diverging** — blue at `min_temp`,
/// neutral white at the *midpoint*, red at `max_temp` — so set the range about a
/// reference that matters rather than clamping it to the extremes seen.
///
/// A recuperator usually sits in the middle of a plant's temperature span, so a
/// plant-wide range renders it in the pale middle of the scale. Narrow the range
/// to the exchanger's own span to see the approach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatExchangerDisplayRange {
    /// Temperature drawn in the coldest displayable colour (blue).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature drawn in the hottest displayable colour (red).
    pub max_temp: ThermodynamicTemperature,
}

/// Scalar state of a heat exchanger, as the caller's own model holds it.
///
/// Every field is **real state the caller already has**, not a placeholder — see
/// the module documentation and
/// [`crate::components::PipeVisual::from_scalars`] for why this narrower
/// interface exists. Nothing here is invented by the widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatExchangerScalars {
    /// Hot stream entering the exchanger.
    ///
    /// Colours the left end of the hot stream and the left inlet nozzle. The hot
    /// stream is always drawn left to right, whatever the arrangement.
    pub hot_inlet_temp: ThermodynamicTemperature,
    /// Hot stream leaving the exchanger, at the right end.
    ///
    /// Below [`Self::hot_inlet_temp`] whenever the exchanger is doing anything.
    pub hot_outlet_temp: ThermodynamicTemperature,
    /// Cold stream entering the exchanger.
    ///
    /// Which **end** that is depends on the arrangement — the right end for
    /// [`HeatExchangerKind::CounterFlow`], the left end for
    /// [`HeatExchangerKind::ParallelFlow`] — which is exactly what the drawing
    /// exists to show.
    pub cold_inlet_temp: ThermodynamicTemperature,
    /// Cold stream leaving the exchanger.
    ///
    /// Above [`Self::cold_inlet_temp`] whenever the exchanger is doing anything,
    /// and — in counter-flow only — possibly above
    /// [`Self::hot_outlet_temp`] as well. See [`ApproachVerdict`].
    pub cold_outlet_temp: ThermodynamicTemperature,
    /// Heat duty transferred between the streams, or `None` when the caller's
    /// model does not have one.
    ///
    /// An [`Option`] deliberately: a caller with four temperatures but no mass
    /// flows has no duty, and a duty label it never computed must not appear.
    /// `None` draws no duty label at all.
    pub duty: Option<Power>,
}

/// Where a [`HeatExchangerVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatExchangerVisualState {
    /// Backed by a [`tampines::components::HeatExchanger`] alone.
    ///
    /// That component holds a flow arrangement, a heat-transfer area and an
    /// overall coefficient — all of which **are** drawn or labelled — and no
    /// fluid state at all. Its `calculate` returns
    /// `TampinesError::NotYetImplemented`, so this path paints no temperature
    /// colour, no approaches and no profile. See the module documentation.
    Physics(HeatExchanger),
    /// Backed by caller-supplied scalars from the caller's own plant model,
    /// graded against the accompanying [`HeatExchangerDisplayRange`].
    Scalars(HeatExchangerScalars, HeatExchangerDisplayRange),
}

/// The colours and values the artwork is actually painted with, with `None`
/// wherever no honest source exists.
///
/// Resolved once per repaint so that every "we do not know this" decision is
/// taken in one place ([`HeatExchangerVisualState::resolve`]) rather than
/// scattered through the drawing code.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DrawnHeatExchanger {
    scalars: Option<HeatExchangerScalars>,
    range: Option<HeatExchangerDisplayRange>,
}

impl DrawnHeatExchanger {
    /// Nothing known: every stream falls back to [`UNKNOWN_FLUID`].
    const UNKNOWN: Self = Self {
        scalars: None,
        range: None,
    };

    /// Colour for a temperature, or [`UNKNOWN_FLUID`] if either the temperature
    /// or the display range is missing.
    fn colour(&self, t: Option<ThermodynamicTemperature>) -> Color32 {
        match (t, self.range) {
            (Some(t), Some(r)) => temperature_colour(t, r.min_temp, r.max_temp),
            _ => UNKNOWN_FLUID,
        }
    }

    /// Temperature of one stream at fraction `f` along **its own path**, `0.0`
    /// at that stream's inlet and `1.0` at its outlet. See [`path_fractions`].
    fn stream_temp(&self, hot: bool, f: f32) -> Option<ThermodynamicTemperature> {
        let s = self.scalars?;
        Some(if hot {
            lerp_temperature(s.hot_inlet_temp, s.hot_outlet_temp, f)
        } else {
            lerp_temperature(s.cold_inlet_temp, s.cold_outlet_temp, f)
        })
    }

    /// Colour of one stream at fraction `f` along its own path.
    fn stream_colour(&self, hot: bool, f: f32) -> Color32 {
        self.colour(self.stream_temp(hot, f))
    }
}

impl HeatExchangerVisualState {
    /// The colours and values the artwork is drawn from.
    ///
    /// [`Self::Physics`] resolves to [`DrawnHeatExchanger::UNKNOWN`] — that is
    /// the whole point of the variant, and it is why the physics path carries
    /// its real flow directions but no colour.
    fn resolve(&self) -> DrawnHeatExchanger {
        match self {
            Self::Physics(_) => DrawnHeatExchanger::UNKNOWN,
            Self::Scalars(s, range) => DrawnHeatExchanger {
                scalars: Some(*s),
                range: Some(*range),
            },
        }
    }
}

// ── The widget ──────────────────────────────────────────────────────────────

/// Visual representation of a two-stream recuperator, in one of two flow
/// arrangements and one of two constructions.
///
/// Built either from a [`tampines::components::HeatExchanger`] ([`Self::new`],
/// which draws the machine's real flow directions and labels its area and `U`
/// but paints no temperature, because that component holds no fluid state) or
/// from the caller's own scalar plant state ([`Self::from_scalars`]). See the
/// module documentation for what each path is allowed to paint.
///
/// The artwork letterboxes to
/// [`HeatExchangerConstruction::native_aspect_ratio`] inside the box it is
/// given, so it never stretches.
pub struct HeatExchangerVisual {
    kind: HeatExchangerKind,
    construction: HeatExchangerConstruction,
    state: HeatExchangerVisualState,
    screen_position: Pos2,
    screen_vector: Vec2,
    show_labels: bool,
    area: Option<Area>,
    overall_coefficient: Option<HeatTransfer>,
}

impl HeatExchangerVisual {
    /// Wrap a [`HeatExchanger`] with the given screen geometry.
    ///
    /// **Signature preserved** from the original placeholder widget, so every
    /// existing call site keeps working unchanged.
    ///
    /// This is the physics-backed path, and it is **not** fully neutral, because
    /// the component is not stateless: its `arrangement` picks the
    /// [`HeatExchangerKind`], so the drawn flow directions, arrows and nozzle
    /// positions are real, and its `area` and `overall_coefficient` are
    /// labelled. What it cannot see is any fluid state — `HeatExchanger::
    /// calculate` returns `TampinesError::NotYetImplemented` — so both streams
    /// are drawn neutral grey, with no approach values and no temperature
    /// profile. Nothing is fabricated to fill the gap; for a coloured exchanger,
    /// pass the state you actually have to [`Self::from_scalars`].
    ///
    /// `screen_position` is the **centre** of the box the artwork is letterboxed
    /// into, and `screen_vector` its size in screen points.
    ///
    /// Defaults to [`HeatExchangerConstruction::ShellAndTube`], which the
    /// component says nothing about; change it with [`Self::with_construction`].
    pub fn new(physics: HeatExchanger, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            kind: HeatExchangerKind::from_arrangement(physics.arrangement),
            construction: HeatExchangerConstruction::ShellAndTube,
            area: Some(physics.area),
            overall_coefficient: Some(physics.overall_coefficient),
            state: HeatExchangerVisualState::Physics(physics),
            screen_position,
            screen_vector,
            show_labels: true,
        }
    }

    /// Build a heat-exchanger visual from the caller's own scalar plant state.
    ///
    /// The state-driven path described in the module documentation: the caller
    /// passes real inlet and outlet temperatures for both streams (and
    /// optionally a duty) from its own model, and the artwork grades itself
    /// against `range`.
    ///
    /// `screen_position` is the centre of the box and `screen_vector` its size
    /// in screen points; the artwork letterboxes to the construction's
    /// proportions inside it. Defaults to
    /// [`HeatExchangerConstruction::ShellAndTube`]; no area or `U` is implied by
    /// this path — attach them with [`Self::with_surface`] if the caller's model
    /// has them.
    pub fn from_scalars(
        kind: HeatExchangerKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        range: HeatExchangerDisplayRange,
        scalars: HeatExchangerScalars,
    ) -> Self {
        Self {
            kind,
            construction: HeatExchangerConstruction::ShellAndTube,
            state: HeatExchangerVisualState::Scalars(scalars, range),
            screen_position,
            screen_vector,
            show_labels: true,
            area: None,
            overall_coefficient: None,
        }
    }

    /// Draw a different flow arrangement with the same state.
    pub fn with_kind(mut self, kind: HeatExchangerKind) -> Self {
        self.kind = kind;
        self
    }

    /// Draw a different construction with the same state.
    pub fn with_construction(mut self, construction: HeatExchangerConstruction) -> Self {
        self.construction = construction;
        self
    }

    /// Label the body with a known heat-transfer area (square metres) and
    /// overall heat-transfer coefficient (watts per square metre kelvin).
    ///
    /// Set automatically by [`Self::new`] from the component's own `area` and
    /// `overall_coefficient` fields. Together these are the `UA` that sets how
    /// close an approach the machine can achieve — real stored state, so they
    /// are labelled even on the path that paints no temperature.
    pub fn with_surface(mut self, area: Area, overall_coefficient: HeatTransfer) -> Self {
        self.area = Some(area);
        self.overall_coefficient = Some(overall_coefficient);
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Which flow arrangement this visual draws.
    pub fn kind(&self) -> HeatExchangerKind {
        self.kind
    }

    /// Which construction this visual draws.
    pub fn construction(&self) -> HeatExchangerConstruction {
        self.construction
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.screen_vector
    }

    /// Where this visual gets its state.
    pub fn state(&self) -> &HeatExchangerVisualState {
        &self.state
    }

    /// The scalar state the artwork is drawn from, or `None` on the
    /// physics-backed path — which has none, and says so rather than
    /// synthesising one.
    pub fn scalars(&self) -> Option<HeatExchangerScalars> {
        match self.state {
            HeatExchangerVisualState::Physics(_) => None,
            HeatExchangerVisualState::Scalars(s, _) => Some(s),
        }
    }

    /// The heat-transfer area, if one is known, in `uom` [`Area`].
    ///
    /// `Some` on the physics path (read from the component) and on any visual
    /// given [`Self::with_surface`]; `None` otherwise.
    pub fn heat_transfer_area(&self) -> Option<Area> {
        self.area
    }

    /// The overall heat-transfer coefficient `U`, if one is known, in `uom`
    /// [`HeatTransfer`] (watts per square metre kelvin).
    pub fn overall_coefficient(&self) -> Option<HeatTransfer> {
        self.overall_coefficient
    }

    /// The heat duty the caller supplied, or `None` on the physics-backed path
    /// and whenever the caller had no duty to give.
    ///
    /// Never computed by the widget from the temperatures — that would need mass
    /// flows and heat capacities the widget does not have.
    pub fn duty(&self) -> Option<Power> {
        self.scalars().and_then(|s| s.duty)
    }

    /// The two terminal approaches, or `None` on the physics-backed path.
    ///
    /// `(left_end, right_end)` as drawn; see [`terminal_approaches`].
    pub fn approaches(&self) -> Option<(TemperatureInterval, TemperatureInterval)> {
        let s = self.scalars()?;
        Some(terminal_approaches(
            self.kind,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        ))
    }

    /// What the supplied temperatures imply about the arrangement, or `None` on
    /// the physics-backed path — which has no temperatures to judge.
    ///
    /// See [`approach_verdict`].
    pub fn verdict(&self) -> Option<ApproachVerdict> {
        let s = self.scalars()?;
        Some(approach_verdict(
            self.kind,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        ))
    }

    /// Draw a small label, unless labels are switched off.
    fn tag(&self, painter: &Painter, at: Pos2, text: &str) {
        self.tag_coloured(painter, at, text, LABEL);
    }

    /// Draw a small label in a given colour, unless labels are switched off.
    fn tag_coloured(&self, painter: &Painter, at: Pos2, text: &str, colour: Color32) {
        if !self.show_labels {
            return;
        }
        painter.text(
            at,
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(8.5),
            colour,
        );
    }
}

impl Widget for HeatExchangerVisual {
    /// Draws the exchanger for [`HeatExchangerVisual::construction`], plumbed
    /// for [`HeatExchangerVisual::kind`]: body, both streams graded along their
    /// own paths, flow arrows, nozzles at the ends the streams really use,
    /// terminal-approach brackets, and the temperature-profile strip.
    ///
    /// Both streams are coloured by temperature through the shared
    /// [`crate::components::temperature_colour`] map. Anything with no honest
    /// source is drawn in neutral [`UNKNOWN_FLUID`] grey — which is both streams
    /// on the physics-backed path, where only the geometry, the arrows and the
    /// area/`U` labels are real.
    fn ui(self, ui: &mut Ui) -> Response {
        let box_rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(box_rect, Sense::hover());
        let painter = ui.painter_at(box_rect);
        let rect = self.construction.fit_native_aspect(box_rect);
        let drawn = self.state.resolve();
        self.draw(&painter, rect, &drawn);
        response
    }
}

impl HeatExchangerVisual {
    /// Draws the whole machine into `rect`.
    ///
    /// Layout, as fractions of `rect` (chosen for legibility, dimensioned from
    /// no design):
    ///
    /// | Feature | Vertical band | Horizontal extent |
    /// |---|---|---|
    /// | hot nozzles | at the hot axis | 0.02 – 0.12 and 0.88 – 0.98 |
    /// | body | 0.16 – 0.62 | 0.12 – 0.88 |
    /// | approach brackets | between the two stream axes | 0.20 and 0.80 |
    /// | profile strip | 0.70 – 0.98 | 0.12 – 0.88 |
    fn draw(&self, painter: &Painter, rect: Rect, drawn: &DrawnHeatExchanger) {
        let w = rect.width();
        let h = rect.height();
        let x = |f: f32| rect.left() + f * w;
        let y = |f: f32| rect.top() + f * h;
        let body = Rect::from_min_max(Pos2::new(x(0.12), y(0.16)), Pos2::new(x(0.88), y(0.62)));

        // Each construction reports where its two streams' axes sit, so the
        // approach brackets and the arrows are placed from the drawing rather
        // than from a second, drift-prone set of constants.
        let (hot_axis, cold_axis) = match self.construction {
            HeatExchangerConstruction::ShellAndTube => {
                self.draw_shell_and_tube(painter, rect, body, drawn)
            }
            HeatExchangerConstruction::PlateFrame => {
                self.draw_plate_pack(painter, rect, body, drawn)
            }
        };

        self.draw_approach_brackets(painter, rect, body, hot_axis, cold_axis);

        let strip = Rect::from_min_max(Pos2::new(x(0.12), y(0.70)), Pos2::new(x(0.88), y(0.965)));
        self.draw_profile_strip(painter, strip, drawn);

        // ── Area and U, when they are known ────────────────────────────────
        if let (Some(a), Some(u)) = (self.area, self.overall_coefficient) {
            self.tag(
                painter,
                Pos2::new(rect.center().x, y(0.655)),
                &format!(
                    "A {:.0} m²  ·  U {:.0} W/(m²·K)",
                    a.get::<square_meter>(),
                    u.get::<watt_per_square_meter_kelvin>()
                ),
            );
        }

        // Silhouette last, so it reads on top.
        painter.rect_stroke(
            body,
            radius(w * 0.012),
            Stroke::new(1.5, OUTLINE),
            StrokeKind::Middle,
        );
    }

    /// Draws a shell-and-tube body: shell, shell-side fill graded along the cold
    /// path, segmental baffles, the tube bundle graded along the hot path,
    /// tubesheets, channel heads, and all four nozzles.
    ///
    /// Returns `(hot_axis_y, cold_axis_y)` — the screen heights the approach
    /// brackets are drawn between.
    ///
    /// The hot stream is in the **tubes** and the cold stream in the **shell**,
    /// which is the usual way round when the hot stream is the dirtier or the
    /// higher-pressure one. The shell-side nozzles move with the arrangement:
    /// the cold inlet is always at the bottom (a shell side stays flooded) and
    /// the cold outlet always at the top, but which *end* each sits at is
    /// exactly what [`HeatExchangerKind`] decides.
    fn draw_shell_and_tube(
        &self,
        painter: &Painter,
        rect: Rect,
        body: Rect,
        drawn: &DrawnHeatExchanger,
    ) -> (f32, f32) {
        let w = rect.width();
        let h = rect.height();
        let hot_axis = body.center().y;
        // The shell-side stream is read off the space above and below the
        // bundle; its bracket end is placed in the lower shell space, which is
        // clear of the tube rows and of the "shell side" caption.
        let cold_axis = body.top() + body.height() * 0.82;

        painter.rect_filled(body, radius(w * 0.012), STEEL);
        let interior = body.shrink(w * 0.006);
        painter.rect_filled(interior, radius(w * 0.010), VOID);

        // ── Shell side, graded along the cold stream's own path ────────────
        let segments = 32;
        for k in 0..segments {
            let s0 = k as f32 / segments as f32;
            let s1 = (k + 1) as f32 / segments as f32;
            let (_, f) = path_fractions(self.kind, 0.5 * (s0 + s1));
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(interior.left() + interior.width() * s0, interior.top()),
                    Pos2::new(
                        interior.left() + interior.width() * s1 + 0.5,
                        interior.bottom(),
                    ),
                ),
                0.0,
                translucent(drawn.stream_colour(false, f), 150),
            );
        }

        // ── Segmental baffles ──────────────────────────────────────────────
        //
        // Alternating top and bottom, which is what forces the shell-side
        // stream to cross the bundle rather than run straight down the shell.
        for k in 0..5 {
            let bx = interior.left() + interior.width() * (k as f32 + 1.0) / 6.0;
            let (y0, y1) = if k % 2 == 0 {
                (interior.top(), interior.top() + interior.height() * 0.70)
            } else {
                (
                    interior.bottom() - interior.height() * 0.70,
                    interior.bottom(),
                )
            };
            painter.line_segment(
                [Pos2::new(bx, y0), Pos2::new(bx, y1)],
                Stroke::new((w * 0.005).max(1.0), INTERNALS),
            );
        }

        // ── Tubesheets and channel heads ───────────────────────────────────
        let tubesheet_left = body.left() + body.width() * 0.08;
        let tubesheet_right = body.right() - body.width() * 0.08;
        for tx in [tubesheet_left, tubesheet_right] {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(tx - w * 0.006, body.top()),
                    Pos2::new(tx + w * 0.006, body.bottom()),
                ),
                0.0,
                FORGING,
            );
        }
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(body.left(), body.top() + body.height() * 0.22),
                Pos2::new(tubesheet_left, body.bottom() - body.height() * 0.22),
            ),
            2.0,
            drawn.stream_colour(true, 0.0),
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(tubesheet_right, body.top() + body.height() * 0.22),
                Pos2::new(body.right(), body.bottom() - body.height() * 0.22),
            ),
            2.0,
            drawn.stream_colour(true, 1.0),
        );

        // ── Tube bundle, graded along the hot stream's own path ────────────
        let bundle_half = body.height() * 0.20;
        let tube_width = (h * 0.014).max(1.2);
        for row in 0..TUBE_ROWS {
            let row_y =
                hot_axis - bundle_half + 2.0 * bundle_half * (row as f32 + 0.5) / TUBE_ROWS as f32;
            self.graded_run(
                painter,
                drawn,
                row_y,
                tubesheet_left,
                tubesheet_right,
                true,
                tube_width,
            );
        }
        self.tag(
            painter,
            Pos2::new(rect.center().x, hot_axis - bundle_half - h * 0.035),
            "tube side (hot)",
        );
        self.tag(
            painter,
            Pos2::new(rect.center().x, body.bottom() - h * 0.045),
            "shell side (cold)",
        );

        // ── Flow arrows ────────────────────────────────────────────────────
        self.flow_arrows(painter, rect, hot_axis, body, true);
        self.flow_arrows(painter, rect, cold_axis, body, false);

        // ── Nozzles ────────────────────────────────────────────────────────
        //
        // Hot: axial, through the channel heads, always left in / right out.
        self.side_nozzle(
            painter,
            rect,
            hot_axis,
            true,
            drawn.stream_colour(true, 0.0),
        );
        self.side_nozzle(
            painter,
            rect,
            hot_axis,
            false,
            drawn.stream_colour(true, 1.0),
        );
        self.tag(
            painter,
            Pos2::new(rect.left() + w * 0.055, hot_axis - h * 0.055),
            "hot in",
        );
        self.tag(
            painter,
            Pos2::new(rect.right() - w * 0.055, hot_axis - h * 0.055),
            "hot out",
        );

        // Cold: radial, on the shell. Inlet at the bottom, outlet at the top;
        // which end each is at is the arrangement.
        let cold_in_x = match self.kind {
            HeatExchangerKind::CounterFlow => body.right() - body.width() * 0.16,
            HeatExchangerKind::ParallelFlow => body.left() + body.width() * 0.16,
        };
        let cold_out_x = match self.kind {
            HeatExchangerKind::CounterFlow => body.left() + body.width() * 0.16,
            HeatExchangerKind::ParallelFlow => body.right() - body.width() * 0.16,
        };
        self.shell_nozzle(
            painter,
            rect,
            body,
            cold_in_x,
            false,
            drawn.stream_colour(false, 0.0),
        );
        self.shell_nozzle(
            painter,
            rect,
            body,
            cold_out_x,
            true,
            drawn.stream_colour(false, 1.0),
        );
        self.tag(
            painter,
            Pos2::new(cold_in_x, body.bottom() + h * 0.075),
            "cold in",
        );
        self.tag(
            painter,
            Pos2::new(cold_out_x, body.top() - h * 0.075),
            "cold out",
        );

        (hot_axis, cold_axis)
    }

    /// Draws a plate-and-frame body: two heavy end frames, a pack of
    /// [`PLATE_CHANNELS`] channels alternating hot and cold, the plate walls
    /// between them, and the corner ports.
    ///
    /// Returns `(hot_axis_y, cold_axis_y)` — the mean heights of the hot and
    /// cold channels, which the approach brackets are drawn between.
    ///
    /// The alternating channels are what make this construction worth drawing:
    /// in [`HeatExchangerKind::CounterFlow`] each channel's arrows point the
    /// opposite way to its neighbours', so the counter-flow is visible as a
    /// stripe pattern rather than as a caption.
    fn draw_plate_pack(
        &self,
        painter: &Painter,
        rect: Rect,
        body: Rect,
        drawn: &DrawnHeatExchanger,
    ) -> (f32, f32) {
        let w = rect.width();
        let h = rect.height();

        painter.rect_filled(body, radius(w * 0.012), STEEL);
        let interior = body.shrink(w * 0.006);
        painter.rect_filled(interior, radius(w * 0.010), VOID);

        // ── End frames ─────────────────────────────────────────────────────
        let frame_w = body.width() * 0.055;
        for fx in [body.left(), body.right() - frame_w] {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(fx, body.top()),
                    Pos2::new(fx + frame_w, body.bottom()),
                ),
                2.0,
                FORGING,
            );
        }
        let pack_left = body.left() + frame_w;
        let pack_right = body.right() - frame_w;

        // ── Channels, alternating hot and cold ─────────────────────────────
        let pack_top = interior.top();
        let pack_h = interior.height();
        let channel_h = pack_h / PLATE_CHANNELS as f32;
        let mut hot_sum = 0.0f32;
        let mut hot_n = 0.0f32;
        let mut cold_sum = 0.0f32;
        let mut cold_n = 0.0f32;
        for c in 0..PLATE_CHANNELS {
            let hot = c % 2 == 0;
            let axis = pack_top + channel_h * (c as f32 + 0.5);
            if hot {
                hot_sum += axis;
                hot_n += 1.0;
            } else {
                cold_sum += axis;
                cold_n += 1.0;
            }
            self.graded_run(
                painter,
                drawn,
                axis,
                pack_left,
                pack_right,
                hot,
                (channel_h * 0.66).max(1.2),
            );
            // The plate wall below this channel.
            if c + 1 < PLATE_CHANNELS {
                painter.line_segment(
                    [
                        Pos2::new(pack_left, pack_top + channel_h * (c as f32 + 1.0)),
                        Pos2::new(pack_right, pack_top + channel_h * (c as f32 + 1.0)),
                    ],
                    Stroke::new(1.0, INTERNALS),
                );
            }
        }
        let hot_axis = hot_sum / hot_n.max(1.0);
        let cold_axis = cold_sum / cold_n.max(1.0);

        // ── Arrows, one run per channel ────────────────────────────────────
        //
        // Drawn per channel rather than once per stream, because the alternating
        // directions are the picture.
        for c in 0..PLATE_CHANNELS {
            let hot = c % 2 == 0;
            let axis = pack_top + channel_h * (c as f32 + 0.5);
            self.flow_arrows(
                painter,
                rect,
                axis,
                Rect::from_min_max(
                    Pos2::new(pack_left, body.top()),
                    Pos2::new(pack_right, body.bottom()),
                ),
                hot,
            );
        }

        // ── Corner ports ───────────────────────────────────────────────────
        //
        // Hot on the upper ports, cold on the lower ones; the cold ports swap
        // ends with the arrangement, the hot ports never do.
        let port_r = (w * 0.014).max(2.0);
        let upper_y = body.top() + body.height() * 0.12;
        let lower_y = body.bottom() - body.height() * 0.12;
        let (left_px, right_px) = (body.left() + frame_w * 0.5, body.right() - frame_w * 0.5);
        painter.circle_filled(
            Pos2::new(left_px, upper_y),
            port_r,
            drawn.stream_colour(true, 0.0),
        );
        painter.circle_filled(
            Pos2::new(right_px, upper_y),
            port_r,
            drawn.stream_colour(true, 1.0),
        );
        let (cold_in_px, cold_out_px) = match self.kind {
            HeatExchangerKind::CounterFlow => (right_px, left_px),
            HeatExchangerKind::ParallelFlow => (left_px, right_px),
        };
        painter.circle_filled(
            Pos2::new(cold_in_px, lower_y),
            port_r,
            drawn.stream_colour(false, 0.0),
        );
        painter.circle_filled(
            Pos2::new(cold_out_px, lower_y),
            port_r,
            drawn.stream_colour(false, 1.0),
        );

        // ── Nozzle stubs off the end frames ────────────────────────────────
        self.side_nozzle(painter, rect, upper_y, true, drawn.stream_colour(true, 0.0));
        self.side_nozzle(
            painter,
            rect,
            upper_y,
            false,
            drawn.stream_colour(true, 1.0),
        );
        self.side_nozzle(
            painter,
            rect,
            lower_y,
            cold_in_px < rect.center().x,
            drawn.stream_colour(false, 0.0),
        );
        self.side_nozzle(
            painter,
            rect,
            lower_y,
            cold_out_px < rect.center().x,
            drawn.stream_colour(false, 1.0),
        );
        self.tag(
            painter,
            Pos2::new(rect.left() + w * 0.05, upper_y - h * 0.055),
            "hot in",
        );
        self.tag(
            painter,
            Pos2::new(rect.right() - w * 0.05, upper_y - h * 0.055),
            "hot out",
        );
        self.tag(
            painter,
            Pos2::new(
                if cold_in_px < rect.center().x {
                    rect.left() + w * 0.05
                } else {
                    rect.right() - w * 0.05
                },
                lower_y + h * 0.055,
            ),
            "cold in",
        );
        self.tag(
            painter,
            Pos2::new(
                if cold_out_px < rect.center().x {
                    rect.left() + w * 0.05
                } else {
                    rect.right() - w * 0.05
                },
                lower_y - h * 0.055,
            ),
            "cold out",
        );
        self.tag(
            painter,
            Pos2::new(rect.center().x, body.bottom() + h * 0.028),
            "alternating plate channels",
        );

        (hot_axis, cold_axis)
    }

    /// Draws one horizontal run of a stream, graded along **that stream's own
    /// path** from its inlet to its outlet.
    ///
    /// A single flat colour would hide the only thing a heat exchanger is there
    /// to do. `hot` picks which stream, and [`path_fractions`] maps drawn
    /// position onto that stream's path, so a counter-flow cold run genuinely
    /// grades the other way round. The gradient is a display interpolation (see
    /// [`lerp_temperature`]), not a computed profile.
    fn graded_run(
        &self,
        painter: &Painter,
        drawn: &DrawnHeatExchanger,
        y: f32,
        x_left: f32,
        x_right: f32,
        hot: bool,
        width: f32,
    ) {
        let segments = 24;
        for k in 0..segments {
            let t0 = k as f32 / segments as f32;
            let t1 = (k + 1) as f32 / segments as f32;
            let x0 = x_left + (x_right - x_left) * t0;
            let x1 = x_left + (x_right - x_left) * t1;
            let (hot_f, cold_f) = path_fractions(self.kind, 0.5 * (t0 + t1));
            let f = if hot { hot_f } else { cold_f };
            painter.line_segment(
                [Pos2::new(x0, y), Pos2::new(x1, y)],
                Stroke::new(width, drawn.stream_colour(hot, f)),
            );
        }
    }

    /// Draws a run of chevrons along `y` inside `span`, pointing the way the
    /// stream actually flows.
    ///
    /// **This is what makes counter-flow visible.** The hot stream always points
    /// right; the cold stream points right in
    /// [`HeatExchangerKind::ParallelFlow`] and left in
    /// [`HeatExchangerKind::CounterFlow`], per
    /// [`HeatExchangerKind::cold_stream_direction`]. The arrows are drawn on the
    /// physics-backed path too, because the arrangement is real state the
    /// component holds — only their colour is withheld.
    fn flow_arrows(&self, painter: &Painter, rect: Rect, y: f32, span: Rect, hot: bool) {
        let w = rect.width();
        let dir = if hot {
            1.0
        } else {
            self.kind.cold_stream_direction()
        };
        let size = (w * 0.011).max(2.0);
        let count = 5;
        for k in 0..count {
            let f = (k as f32 + 0.5) / count as f32;
            let cx = span.left() + span.width() * f;
            let tip = cx + dir * size;
            let tail = cx - dir * size;
            let stroke = Stroke::new(1.4, Color32::from_rgba_unmultiplied(20, 22, 26, 210));
            painter.line_segment([Pos2::new(tail, y - size), Pos2::new(tip, y)], stroke);
            painter.line_segment([Pos2::new(tail, y + size), Pos2::new(tip, y)], stroke);
        }
    }

    /// A nozzle stub running horizontally out of the body at height `y`.
    ///
    /// `from_left` puts it on the left-hand side.
    fn side_nozzle(&self, painter: &Painter, rect: Rect, y: f32, from_left: bool, colour: Color32) {
        let w = rect.width();
        let h = rect.height();
        let half = (h * 0.022).max(1.5);
        let (x0, x1) = if from_left {
            (rect.left() + w * 0.02, rect.left() + w * 0.14)
        } else {
            (rect.right() - w * 0.14, rect.right() - w * 0.02)
        };
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0, y - half), Pos2::new(x1, y + half)),
            2.0,
            colour,
        );
    }

    /// A nozzle stub running vertically out of the shell at abscissa `x`.
    ///
    /// `on_top` puts it above the shell (the outlet), otherwise below it (the
    /// inlet).
    fn shell_nozzle(
        &self,
        painter: &Painter,
        rect: Rect,
        body: Rect,
        x: f32,
        on_top: bool,
        colour: Color32,
    ) {
        let w = rect.width();
        let h = rect.height();
        let half = (w * 0.014).max(1.5);
        let (y0, y1) = if on_top {
            (body.top() - h * 0.075, body.top() + h * 0.01)
        } else {
            (body.bottom() - h * 0.01, body.bottom() + h * 0.075)
        };
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x - half, y0), Pos2::new(x + half, y1)),
            2.0,
            colour,
        );
    }

    /// Draws the terminal-approach brackets at each end of the body, and the
    /// verdict tag when the supplied temperatures are worth remarking on.
    ///
    /// Each bracket spans from the hot stream's axis to the cold stream's axis
    /// and is labelled with the temperature difference **at that end** — which
    /// is the pair [`terminal_approaches`] returns, and therefore the pair the
    /// log-mean is formed from. When the arrangement changes, the two labels
    /// swap which cold-stream terminal they refer to without the bracket moving,
    /// which is the point: the ends are fixed, the streams are not.
    ///
    /// On the physics-backed path there are no temperatures, so the brackets are
    /// drawn as hatched, unlabelled ticks rather than given a number.
    fn draw_approach_brackets(
        &self,
        painter: &Painter,
        rect: Rect,
        body: Rect,
        hot_axis: f32,
        cold_axis: f32,
    ) {
        let w = rect.width();
        let h = rect.height();
        let (top, bottom) = (hot_axis.min(cold_axis), hot_axis.max(cold_axis));
        let ends = [
            (body.left() + body.width() * 0.11, true),
            (body.right() - body.width() * 0.11, false),
        ];

        let approaches = self.approaches();
        for (bx, is_left) in ends {
            let colour = match approaches {
                Some(_) => translucent(LABEL, 190),
                None => translucent(UNKNOWN_FLUID, 110),
            };
            painter.line_segment(
                [Pos2::new(bx, top), Pos2::new(bx, bottom)],
                Stroke::new(1.2, colour),
            );
            for cap in [top, bottom] {
                painter.line_segment(
                    [
                        Pos2::new(bx - w * 0.008, cap),
                        Pos2::new(bx + w * 0.008, cap),
                    ],
                    Stroke::new(1.2, colour),
                );
            }
            if let Some((left, right)) = approaches {
                let dt = if is_left { left } else { right };
                let dt_k = dt.get::<kelvin_interval>();
                let text_colour = if dt_k > 0.0 { LABEL } else { WARNING };
                self.tag_coloured(
                    painter,
                    Pos2::new(bx, 0.5 * (top + bottom)),
                    &format!("ΔT {dt_k:.1} K"),
                    text_colour,
                );
            }
        }

        // ── Verdict ────────────────────────────────────────────────────────
        match self.verdict() {
            Some(ApproachVerdict::TemperatureCross) => self.tag_coloured(
                painter,
                Pos2::new(rect.center().x, rect.top() + h * 0.035),
                "temperature cross — cold out above hot out (counter-flow only)",
                LABEL,
            ),
            Some(ApproachVerdict::Impossible) => self.tag_coloured(
                painter,
                Pos2::new(rect.center().x, rect.top() + h * 0.035),
                "⚠ impossible for this arrangement — an approach is not positive",
                WARNING,
            ),
            Some(ApproachVerdict::Feasible) => {}
            None => self.tag_coloured(
                painter,
                Pos2::new(rect.center().x, rect.top() + h * 0.035),
                "no fluid state — flow directions real, temperatures unknown",
                translucent(UNKNOWN_FLUID, 220),
            ),
        }
    }

    /// Draws the temperature-profile strip under the body.
    ///
    /// Two lines against length: the hot stream falling left to right, and the
    /// cold stream, which in [`HeatExchangerKind::CounterFlow`] also falls left
    /// to right (because it is *leaving* at the left) and in
    /// [`HeatExchangerKind::ParallelFlow`] rises. That is the whole distinction
    /// between the arrangements, drawn as two lines:
    ///
    /// - parallel flow — the lines start far apart at the left and converge
    ///   toward a common temperature, and the cold line can never end above the
    ///   hot line;
    /// - counter-flow — the lines run roughly parallel down the length, and the
    ///   cold line's left-hand end (its outlet) **can** sit above the hot line's
    ///   right-hand end (its outlet).
    ///
    /// Each line is drawn segment by segment in its own temperature colour, so
    /// the strip and the body agree. The vertical scale comes from
    /// [`profile_temperature_bounds`] — the four terminal temperatures, not the
    /// display range — while the colour still comes from the display range.
    ///
    /// On the physics-backed path there is nothing to plot, so the strip is
    /// drawn as an empty framed axis with a caption saying so. It is deliberately
    /// not left out: an absent panel would look like a layout choice, whereas an
    /// empty one is a statement.
    fn draw_profile_strip(&self, painter: &Painter, strip: Rect, drawn: &DrawnHeatExchanger) {
        if strip.width() <= 0.0 || strip.height() <= 0.0 {
            return;
        }
        painter.rect_filled(strip, 2.0, Color32::from_rgba_unmultiplied(18, 20, 24, 170));
        painter.rect_stroke(
            strip,
            2.0,
            Stroke::new(1.0, translucent(OUTLINE, 120)),
            StrokeKind::Middle,
        );

        let Some(scalars) = drawn.scalars else {
            self.tag_coloured(
                painter,
                strip.center(),
                "no temperature profile — the component holds no fluid state",
                translucent(UNKNOWN_FLUID, 220),
            );
            return;
        };

        let (lo, hi) = profile_temperature_bounds(&scalars);
        let (lo_k, hi_k) = (lo.get::<kelvin>(), hi.get::<kelvin>());
        let span = (hi_k - lo_k).max(1e-9);
        let plot = strip.shrink2(Vec2::new(strip.width() * 0.06, strip.height() * 0.18));
        let y_of = |t: f64| plot.bottom() - ((t - lo_k) / span) as f32 * plot.height();

        let segments = 40;
        for hot in [false, true] {
            let width = if hot { 2.0 } else { 1.8 };
            for k in 0..segments {
                let s0 = k as f32 / segments as f32;
                let s1 = (k + 1) as f32 / segments as f32;
                let (h0, c0) = path_fractions(self.kind, s0);
                let (h1, c1) = path_fractions(self.kind, s1);
                let (f0, f1) = if hot { (h0, h1) } else { (c0, c1) };
                let (Some(t0), Some(t1)) = (drawn.stream_temp(hot, f0), drawn.stream_temp(hot, f1))
                else {
                    continue;
                };
                painter.line_segment(
                    [
                        Pos2::new(plot.left() + plot.width() * s0, y_of(t0.get::<kelvin>())),
                        Pos2::new(plot.left() + plot.width() * s1, y_of(t1.get::<kelvin>())),
                    ],
                    Stroke::new(width, drawn.stream_colour(hot, 0.5 * (f0 + f1))),
                );
            }
        }

        // Duty, when the caller has one. Never derived from the temperatures.
        if let Some(duty) = scalars.duty {
            self.tag(
                painter,
                Pos2::new(strip.center().x, strip.top() + strip.height() * 0.13),
                &format!("Q {:.0} kW", duty.get::<kilowatt>()),
            );
        }
        self.tag_coloured(
            painter,
            Pos2::new(strip.center().x, strip.bottom() - strip.height() * 0.11),
            "temperature along the length (endpoints exact, path drawn straight)",
            translucent(LABEL, 170),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd;
    use uom::si::power::kilowatt as kilowatt_unit;
    use uom::si::thermodynamic_temperature::degree_celsius;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn range() -> HeatExchangerDisplayRange {
        HeatExchangerDisplayRange {
            min_temp: degc(20.0),
            max_temp: degc(220.0),
        }
    }

    /// Illustrative state for a liquid-liquid recuperator: hot stream cooling
    /// 180 -> 95 degC against a cold stream heating 40 -> 110 degC, 1 200 kW.
    /// Demonstration values only — note the cold outlet (110) is above the hot
    /// outlet (95), which is a temperature cross and therefore counter-flow
    /// only.
    fn scalars() -> HeatExchangerScalars {
        HeatExchangerScalars {
            hot_inlet_temp: degc(180.0),
            hot_outlet_temp: degc(95.0),
            cold_inlet_temp: degc(40.0),
            cold_outlet_temp: degc(110.0),
            duty: Some(Power::new::<kilowatt_unit>(1200.0)),
        }
    }

    fn visual(kind: HeatExchangerKind) -> HeatExchangerVisual {
        HeatExchangerVisual::from_scalars(
            kind,
            Pos2::new(100.0, 200.0),
            Vec2::new(360.0, 200.0),
            range(),
            scalars(),
        )
    }

    /// Each construction must keep its own proportions at any box size.
    ///
    /// **Methodology.** A shell-and-tube unit is long and slim and a plate pack
    /// is squat; letting either stretch to fill its card would make them read as
    /// the same machine, which defeats drawing two constructions. Require
    /// [`HeatExchangerConstruction::native_aspect_ratio`] to equal the module
    /// constants, require the shell-and-tube ratio to exceed the plate ratio,
    /// and require [`HeatExchangerConstruction::fit_native_aspect`] to preserve
    /// the ratio, stay centred, and never overflow, in a square box, an
    /// over-wide box and an over-tall box.
    ///
    /// **Result (2026-08-12):** ratios 2.05 (shell and tube) and 1.35 (plate and
    /// frame); all six box/construction combinations preserved the ratio to
    /// better than 1e-4, stayed centred to better than 1e-4 points, and never
    /// exceeded the box — shell-and-tube 300x300 -> 300.0x146.3, 900x200 ->
    /// 410.0x200.0, 100x900 -> 100.0x48.8; plate 300x300 -> 300.0x222.2,
    /// 900x200 -> 270.0x200.0, 100x900 -> 100.0x74.1. Interpretation: the two
    /// constructions stay visually distinct in any gallery cell.
    #[test]
    fn each_construction_letterboxes_to_its_native_proportions() {
        assert!(
            (HeatExchangerConstruction::ShellAndTube.native_aspect_ratio()
                - SHELL_AND_TUBE_ASPECT_RATIO)
                .abs()
                < 1e-6
        );
        assert!(
            (HeatExchangerConstruction::PlateFrame.native_aspect_ratio()
                - PLATE_FRAME_ASPECT_RATIO)
                .abs()
                < 1e-6
        );
        assert!(
            SHELL_AND_TUBE_ASPECT_RATIO > PLATE_FRAME_ASPECT_RATIO,
            "a shell-and-tube unit must read as the slimmer machine"
        );

        for construction in HeatExchangerConstruction::ALL {
            for size in [
                Vec2::new(300.0, 300.0),
                Vec2::new(900.0, 200.0),
                Vec2::new(100.0, 900.0),
            ] {
                let box_rect = Rect::from_min_size(Pos2::new(17.0, 23.0), size);
                let fitted = construction.fit_native_aspect(box_rect);
                println!(
                    "{:?} in {size:?} -> {:.1}x{:.1}",
                    construction,
                    fitted.width(),
                    fitted.height()
                );
                assert!(
                    (fitted.width() / fitted.height() - construction.native_aspect_ratio()).abs()
                        < 1e-4,
                    "{construction:?} in box {size:?} did not preserve its ratio"
                );
                assert!(
                    (fitted.center() - box_rect.center()).length() < 1e-4,
                    "{construction:?} in box {size:?} was not centred"
                );
                assert!(
                    fitted.width() <= size.x + 1e-3 && fitted.height() <= size.y + 1e-3,
                    "{construction:?} in box {size:?} overflowed its box"
                );
            }
        }
    }

    /// A degenerate box must not produce NaN geometry — zero-height allocations
    /// happen transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        for construction in HeatExchangerConstruction::ALL {
            assert_eq!(construction.fit_native_aspect(flat), flat);
        }
    }

    /// The two streams must be drawn running the way the arrangement says, and
    /// counter-flow must genuinely reverse the cold stream.
    ///
    /// **Methodology.** The hot stream is always drawn left to right, so
    /// [`path_fractions`] must return `s` for it at every position. For the cold
    /// stream, require the parallel-flow fraction to equal `s` (same direction)
    /// and the counter-flow fraction to equal `1 - s` (opposite), require both
    /// to stay in `[0, 1]`, require out-of-range positions to clamp rather than
    /// extrapolate, and require the two arrangements to disagree everywhere
    /// except at mid-length — where they must agree exactly, since that is the
    /// one point both conventions place at half the cold path. Cross-check
    /// against [`HeatExchangerKind::cold_stream_direction`]. Swept over 201
    /// positions from -0.5 to 1.5.
    ///
    /// **Result (2026-08-12):** 202 in-range fraction pairs evaluated over both
    /// kinds (101 positions in `[0, 1]`, two kinds each); every fraction inside
    /// `[0, 1]` at all 201 swept positions; the hot fraction equalled the
    /// clamped position exactly everywhere; the counter-flow and parallel-flow
    /// cold fractions differed at 100 of the 101 in-range positions and agreed
    /// only at s = 0.5, both giving 0.5; positions -0.5 and 1.5 clamped to the
    /// s = 0 and s = 1 results. `cold_stream_direction` gave -1.0 for
    /// counter-flow and +1.0 for parallel flow. Interpretation: the opposition
    /// of the two streams is in the geometry, so the arrows and the gradients
    /// cannot disagree with the caption.
    #[test]
    fn the_two_streams_run_the_way_the_arrangement_says() {
        let mut evaluated = 0usize;
        let mut differing = 0usize;
        for step in -50..=150 {
            let s = step as f32 * 0.01;
            let clamped = s.clamp(0.0, 1.0);
            let (hot_cf, cold_cf) = path_fractions(HeatExchangerKind::CounterFlow, s);
            let (hot_pf, cold_pf) = path_fractions(HeatExchangerKind::ParallelFlow, s);
            assert_eq!(
                hot_cf, clamped,
                "the hot stream is always drawn left to right"
            );
            assert_eq!(hot_pf, clamped);
            assert!(
                (cold_pf - clamped).abs() < 1e-6,
                "parallel flow runs together"
            );
            assert!(
                (cold_cf - (1.0 - clamped)).abs() < 1e-6,
                "counter-flow runs against"
            );
            for f in [hot_cf, cold_cf, hot_pf, cold_pf] {
                assert!(
                    (0.0..=1.0).contains(&f),
                    "fraction {f} left [0, 1] at s = {s}"
                );
            }
            if (0.0..=1.0).contains(&s) {
                if (cold_cf - cold_pf).abs() > 1e-6 {
                    differing += 1;
                }
                evaluated += 2;
            }
        }
        println!("{evaluated} fraction pairs evaluated, {differing} cold fractions differing");
        assert_eq!(
            differing, 100,
            "the two arrangements must agree only at mid-length"
        );
        assert_eq!(
            path_fractions(HeatExchangerKind::CounterFlow, 0.5),
            path_fractions(HeatExchangerKind::ParallelFlow, 0.5)
        );
        assert_eq!(
            path_fractions(HeatExchangerKind::CounterFlow, -0.5),
            path_fractions(HeatExchangerKind::CounterFlow, 0.0)
        );
        assert_eq!(
            path_fractions(HeatExchangerKind::CounterFlow, 1.5),
            path_fractions(HeatExchangerKind::CounterFlow, 1.0)
        );
        assert_eq!(HeatExchangerKind::CounterFlow.cold_stream_direction(), -1.0);
        assert_eq!(HeatExchangerKind::ParallelFlow.cold_stream_direction(), 1.0);
    }

    /// The end brackets must be labelled with the same two temperature
    /// differences the log-mean is formed from, or the drawing and the rating
    /// would be describing different machines.
    ///
    /// **Methodology.** [`terminal_approaches`] is the widget's own convention;
    /// [`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd`] is the
    /// workspace's ported DWSIM rating formula. Take the illustrative operating
    /// point (hot 180 -> 95 degC, cold 40 -> 110 degC), form `(dt1, dt2)` from
    /// the widget's brackets for each arrangement, and require
    /// `(dt1 - dt2)/ln(dt1/dt2)` to reproduce `lmtd(...)` for that arrangement
    /// to within 1e-6 K. Then require the counter-flow brackets to be the
    /// specific pairing `(T_hot_in - T_cold_out, T_hot_out - T_cold_in)` and the
    /// parallel-flow brackets `(T_hot_in - T_cold_in, T_hot_out - T_cold_out)`.
    ///
    /// **Result (2026-08-12):** counter-flow brackets 70.0000 K (left) and
    /// 55.0000 K (right), reconstructing an LMTD of 62.1988 K against
    /// `lmtd()`'s 62.1988 K, agreeing to better than 1e-6 K; parallel-flow
    /// brackets 140.0000 K and -15.0000 K, whose ratio is negative — so no real
    /// log-mean exists, which is itself the correct answer for an operating
    /// point parallel flow cannot reach, and the pairing was checked term by
    /// term instead. Interpretation: the bracket labels are the log-mean's own
    /// two ends, so a reader can take the numbers off the picture and reproduce
    /// the rating.
    #[test]
    fn the_end_brackets_are_the_two_ends_of_the_log_mean() {
        let s = scalars();
        for kind in HeatExchangerKind::ALL {
            let (left, right) = terminal_approaches(
                *kind,
                s.hot_inlet_temp,
                s.hot_outlet_temp,
                s.cold_inlet_temp,
                s.cold_outlet_temp,
            );
            let (dt1, dt2) = (
                left.get::<kelvin_interval>(),
                right.get::<kelvin_interval>(),
            );
            println!("{kind:?}: dt1 = {dt1:.4} K, dt2 = {dt2:.4} K");
            if dt1 > 0.0 && dt2 > 0.0 && (dt1 - dt2).abs() > 1e-9 {
                let ours = (dt1 - dt2) / (dt1 / dt2).ln();
                let theirs = lmtd(
                    kind.arrangement(),
                    s.hot_inlet_temp,
                    s.hot_outlet_temp,
                    s.cold_inlet_temp,
                    s.cold_outlet_temp,
                )
                .get::<kelvin_interval>();
                println!("  reconstructed LMTD {ours:.4} K vs lmtd() {theirs:.4} K");
                assert!(
                    (ours - theirs).abs() < 1e-6,
                    "{kind:?} brackets do not reproduce the log-mean"
                );
            }
        }

        // Term-by-term pairing, which holds whether or not the log-mean is real.
        let (cl, cr) = terminal_approaches(
            HeatExchangerKind::CounterFlow,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        );
        assert!((cl.get::<kelvin_interval>() - 70.0).abs() < 1e-9);
        assert!((cr.get::<kelvin_interval>() - 55.0).abs() < 1e-9);

        let (pl, pr) = terminal_approaches(
            HeatExchangerKind::ParallelFlow,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        );
        assert!((pl.get::<kelvin_interval>() - 140.0).abs() < 1e-9);
        assert!((pr.get::<kelvin_interval>() + 15.0).abs() < 1e-9);
    }

    /// **Parallel flow must never be able to put the cold outlet above the hot
    /// outlet, and counter-flow must be able to.** This is the one physical
    /// claim the widget makes, so it is pinned by a sweep rather than asserted
    /// in prose.
    ///
    /// **Methodology.** Fix the hot stream at 180 -> 95 degC and sweep the cold
    /// stream's inlet from 10 to 175 degC and its outlet from 15 to 200 degC in
    /// 5 K steps, keeping only physically ordered cold streams (outlet above
    /// inlet). At every point, evaluate [`approach_verdict`] for both
    /// arrangements and require: parallel flow **never** returns
    /// [`ApproachVerdict::TemperatureCross`]; every crossed pair
    /// (`T_cold_out > T_hot_out`) offered to parallel flow returns
    /// [`ApproachVerdict::Impossible`]; counter-flow returns
    /// `TemperatureCross` for at least one point; and every `TemperatureCross`
    /// verdict has both terminal approaches strictly positive, so it is a
    /// genuine operating point and not a disguised violation. Also require
    /// non-finite temperatures to give `Impossible`.
    ///
    /// **Result (2026-08-12):** 731 ordered cold streams sampled (1 462
    /// verdicts). Parallel flow returned `TemperatureCross` **0** times, and all
    /// 578 crossed pairs offered to it returned `Impossible`. Counter-flow
    /// returned `Feasible` 153 times, `TemperatureCross` 272 times and
    /// `Impossible` 306 times, and every one of the 272 crossings had both
    /// terminal approaches strictly positive — the smallest seen at any crossing
    /// was 5.0 K. A NaN cold outlet gave `Impossible` for both arrangements.
    /// Interpretation: the widget cannot be made to draw a temperature cross on
    /// a parallel-flow machine, which is exactly the lesson the studio tab is
    /// built to demonstrate.
    #[test]
    fn only_counter_flow_can_cross_the_outlet_temperatures() {
        let (hot_in, hot_out) = (degc(180.0), degc(95.0));
        let mut sampled = 0usize;
        let mut crossed_pairs_offered_to_parallel = 0usize;
        let mut counter = (0usize, 0usize, 0usize); // feasible, cross, impossible
        let mut parallel_crosses = 0usize;
        let mut smallest_cross_approach = f64::INFINITY;

        for cold_in_step in 0..=33 {
            let cold_in = degc(10.0 + cold_in_step as f64 * 5.0);
            for cold_out_step in 0..=37 {
                let cold_out = degc(15.0 + cold_out_step as f64 * 5.0);
                if cold_out.get::<kelvin>() <= cold_in.get::<kelvin>() {
                    continue;
                }
                sampled += 1;
                let is_crossed = cold_out.get::<kelvin>() > hot_out.get::<kelvin>();

                let pf = approach_verdict(
                    HeatExchangerKind::ParallelFlow,
                    hot_in,
                    hot_out,
                    cold_in,
                    cold_out,
                );
                if pf == ApproachVerdict::TemperatureCross {
                    parallel_crosses += 1;
                }
                if is_crossed {
                    crossed_pairs_offered_to_parallel += 1;
                    assert_eq!(
                        pf,
                        ApproachVerdict::Impossible,
                        "parallel flow accepted a crossed pair: cold {:?} -> {:?}",
                        cold_in.get::<kelvin>(),
                        cold_out.get::<kelvin>()
                    );
                }

                let cf = approach_verdict(
                    HeatExchangerKind::CounterFlow,
                    hot_in,
                    hot_out,
                    cold_in,
                    cold_out,
                );
                match cf {
                    ApproachVerdict::Feasible => counter.0 += 1,
                    ApproachVerdict::TemperatureCross => {
                        counter.1 += 1;
                        let (l, r) = terminal_approaches(
                            HeatExchangerKind::CounterFlow,
                            hot_in,
                            hot_out,
                            cold_in,
                            cold_out,
                        );
                        let (l, r) = (l.get::<kelvin_interval>(), r.get::<kelvin_interval>());
                        assert!(
                            l > 0.0 && r > 0.0,
                            "a temperature cross must still have positive approaches"
                        );
                        smallest_cross_approach = smallest_cross_approach.min(l.min(r));
                    }
                    ApproachVerdict::Impossible => counter.2 += 1,
                }
            }
        }

        println!(
            "{sampled} ordered cold streams sampled ({} verdicts)",
            sampled * 2
        );
        println!(
            "counter-flow: {} feasible, {} crossed, {} impossible",
            counter.0, counter.1, counter.2
        );
        println!(
            "parallel flow: {parallel_crosses} crossings, \
             {crossed_pairs_offered_to_parallel} crossed pairs all rejected"
        );
        println!("smallest approach at a crossing: {smallest_cross_approach:.1} K");

        assert_eq!(
            parallel_crosses, 0,
            "parallel flow must never report a temperature cross"
        );
        assert!(counter.1 > 0, "counter-flow must be able to cross");
        assert!(
            crossed_pairs_offered_to_parallel > 0,
            "the sweep must exercise crossings"
        );
        assert!(HeatExchangerKind::CounterFlow.permits_temperature_cross());
        assert!(!HeatExchangerKind::ParallelFlow.permits_temperature_cross());

        // A NaN must be the most visible outcome, not a plausible label.
        let nan = ThermodynamicTemperature::new::<kelvin>(f64::NAN);
        for kind in HeatExchangerKind::ALL {
            assert_eq!(
                approach_verdict(*kind, hot_in, hot_out, degc(40.0), nan),
                ApproachVerdict::Impossible
            );
        }
    }

    /// The profile strip must be scaled to the four terminal temperatures, and
    /// must never collapse to a line or divide by zero.
    ///
    /// **Methodology.** Require [`profile_temperature_bounds`] to bracket all
    /// four supplied temperatures with a margin at both ends, to widen a
    /// degenerate window (all four equal, which is what zero duty gives) to at
    /// least 1 K, and to return a finite, ordered window at least 1 K wide when
    /// every temperature is non-finite.
    ///
    /// **Result (2026-08-12):** the illustrative point (40, 95, 110, 180 degC)
    /// gave a window of 23.20 -> 196.80 degC, span 173.60 K against a 140.00 K
    /// temperature range, i.e. a 12 % margin at each end; an all-equal 100 degC
    /// state gave 99.50 -> 100.50 degC, span exactly 1.00 K; an all-NaN state
    /// gave 272.65 -> 273.65 K, also 1.00 K. Interpretation: a 20 K approach
    /// stays legible on the strip even when the colour scale spans a whole
    /// plant, and no operating point can make the strip degenerate.
    #[test]
    fn the_profile_strip_is_scaled_to_the_terminal_temperatures() {
        let s = scalars();
        let (lo, hi) = profile_temperature_bounds(&s);
        println!(
            "window {:.2} -> {:.2} degC",
            lo.get::<degree_celsius>(),
            hi.get::<degree_celsius>()
        );
        assert!(lo.get::<kelvin>() < s.cold_inlet_temp.get::<kelvin>());
        assert!(hi.get::<kelvin>() > s.hot_inlet_temp.get::<kelvin>());
        assert!(hi.get::<kelvin>() - lo.get::<kelvin>() > 140.0);

        let flat = HeatExchangerScalars {
            hot_inlet_temp: degc(100.0),
            hot_outlet_temp: degc(100.0),
            cold_inlet_temp: degc(100.0),
            cold_outlet_temp: degc(100.0),
            duty: None,
        };
        let (lo, hi) = profile_temperature_bounds(&flat);
        println!(
            "flat window {:.2} -> {:.2} degC",
            lo.get::<degree_celsius>(),
            hi.get::<degree_celsius>()
        );
        assert!(
            hi.get::<kelvin>() - lo.get::<kelvin>() >= 1.0 - 1e-9,
            "the strip must not collapse"
        );

        let nan = ThermodynamicTemperature::new::<kelvin>(f64::NAN);
        let broken = HeatExchangerScalars {
            hot_inlet_temp: nan,
            hot_outlet_temp: nan,
            cold_inlet_temp: nan,
            cold_outlet_temp: nan,
            duty: None,
        };
        let (lo, hi) = profile_temperature_bounds(&broken);
        assert!(lo.get::<kelvin>().is_finite() && hi.get::<kelvin>().is_finite());
        assert!(hi.get::<kelvin>() - lo.get::<kelvin>() >= 1.0 - 1e-9);
    }

    /// The physics-backed path must paint no temperature it cannot see — but it
    /// **must** draw the arrangement, area and `U` it really holds.
    ///
    /// **Methodology.** `tampines::components::HeatExchanger` stores a flow
    /// arrangement, a heat-transfer area and an overall coefficient, and its
    /// `calculate` returns `NotYetImplemented`. Wrap one with
    /// [`HeatExchangerVisual::new`] — the preserved three-argument signature —
    /// and require: the resolved drawing state to carry no scalars and no
    /// display range; every resolved stream colour to be [`UNKNOWN_FLUID`]
    /// rather than a point on the temperature scale; the approaches, the verdict
    /// and the duty all to be `None`; and, conversely, the drawn
    /// [`HeatExchangerKind`] to match the component's own `arrangement` for
    /// **both** arrangements, with the area and coefficient surviving as real
    /// stored state.
    ///
    /// **Result (2026-08-12):** a counter-current exchanger of 240 m² at
    /// 850 W/(m²·K) resolved to `DrawnHeatExchanger::UNKNOWN`; both stream
    /// colours at path fractions 0.0, 0.5 and 1.0 were `Color32::GRAY`
    /// (160, 160, 160), which the diverging temperature map never produces;
    /// `scalars()`, `approaches()`, `verdict()` and `duty()` were all `None`;
    /// `kind()` came back `CounterFlow` for a `CounterCurrent` component and
    /// `ParallelFlow` for a `CoCurrent` one; `heat_transfer_area()` returned
    /// 240 m² and `overall_coefficient()` 850 W/(m²·K). Interpretation: the
    /// neutral path is neutral about fluid state only. A future change that
    /// starts fabricating a temperature fails here, and so does one that stops
    /// honouring the arrangement the component really stores.
    #[test]
    fn the_physics_path_draws_its_arrangement_but_paints_no_temperature() {
        let area = Area::new::<square_meter>(240.0);
        let u = HeatTransfer::new::<watt_per_square_meter_kelvin>(850.0);
        let physics = HeatExchanger::new(FlowArrangement::CounterCurrent, area, u);
        let visual =
            HeatExchangerVisual::new(physics, Pos2::new(0.0, 0.0), Vec2::new(360.0, 200.0));

        let drawn = visual.state.resolve();
        assert_eq!(drawn, DrawnHeatExchanger::UNKNOWN);
        assert!(drawn.scalars.is_none());
        assert!(drawn.range.is_none());
        for hot in [true, false] {
            for f in [0.0, 0.5, 1.0] {
                assert_eq!(drawn.stream_colour(hot, f), UNKNOWN_FLUID);
                assert!(drawn.stream_temp(hot, f).is_none());
            }
        }
        // Even with a temperature in hand, no display range means no colour.
        assert_eq!(drawn.colour(Some(degc(120.0))), UNKNOWN_FLUID);

        assert!(visual.scalars().is_none());
        assert!(visual.approaches().is_none());
        assert!(visual.verdict().is_none());
        assert!(visual.duty().is_none());

        // What it DOES know is drawn.
        assert_eq!(visual.kind(), HeatExchangerKind::CounterFlow);
        assert_eq!(
            visual.construction(),
            HeatExchangerConstruction::ShellAndTube
        );
        assert_eq!(visual.heat_transfer_area(), Some(area));
        assert_eq!(visual.overall_coefficient(), Some(u));
        assert_eq!(visual.size(), Vec2::new(360.0, 200.0));

        let co_current = HeatExchangerVisual::new(
            HeatExchanger::new(FlowArrangement::CoCurrent, area, u),
            Pos2::ZERO,
            Vec2::new(360.0, 200.0),
        );
        assert_eq!(co_current.kind(), HeatExchangerKind::ParallelFlow);
    }

    /// The arrangement must survive a round trip through the physics enum, so
    /// the studio's selection and a rating call cannot disagree.
    #[test]
    fn the_arrangement_round_trips_through_the_physics_enum() {
        for kind in HeatExchangerKind::ALL {
            assert_eq!(
                HeatExchangerKind::from_arrangement(kind.arrangement()),
                *kind
            );
        }
        assert_eq!(
            HeatExchangerKind::CounterFlow.arrangement(),
            FlowArrangement::CounterCurrent
        );
        assert_eq!(
            HeatExchangerKind::ParallelFlow.arrangement(),
            FlowArrangement::CoCurrent
        );
    }

    /// The scalar path must pass the caller's state through untouched — this is
    /// real model state, not a placeholder, so nothing may be substituted.
    #[test]
    fn the_scalar_path_passes_state_through_unchanged() {
        for kind in HeatExchangerKind::ALL {
            let v = visual(*kind);
            assert_eq!(v.scalars(), Some(scalars()));
            assert_eq!(v.kind(), *kind);
            assert_eq!(v.size(), Vec2::new(360.0, 200.0));
            // The scalar path implies no area or coefficient until it is given.
            assert!(v.heat_transfer_area().is_none());
            assert!(v.overall_coefficient().is_none());
            assert_eq!(v.duty(), Some(Power::new::<kilowatt_unit>(1200.0)));
        }

        let with_surface = visual(HeatExchangerKind::CounterFlow).with_surface(
            Area::new::<square_meter>(240.0),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(850.0),
        );
        assert_eq!(
            with_surface.heat_transfer_area(),
            Some(Area::new::<square_meter>(240.0))
        );
        assert_eq!(
            with_surface.construction(),
            HeatExchangerConstruction::ShellAndTube
        );
        assert_eq!(
            with_surface
                .with_construction(HeatExchangerConstruction::PlateFrame)
                .construction(),
            HeatExchangerConstruction::PlateFrame
        );
    }

    /// A caller with no duty must get no duty label — a missing measurement and
    /// a zero measurement are different claims.
    #[test]
    fn a_missing_duty_stays_missing() {
        let mut s = scalars();
        s.duty = None;
        let v = HeatExchangerVisual::from_scalars(
            HeatExchangerKind::CounterFlow,
            Pos2::ZERO,
            Vec2::new(360.0, 200.0),
            range(),
            s,
        );
        assert!(v.duty().is_none());
        // Everything else it *was* given still draws.
        assert!(v.approaches().is_some());
        assert_ne!(v.state.resolve().stream_colour(true, 0.0), UNKNOWN_FLUID);
    }

    /// The scalar path must colour both streams from the caller's own numbers,
    /// and each stream must visibly change temperature along its own path.
    ///
    /// **Methodology.** Resolve the drawing state from the illustrative scalars
    /// (hot 180 -> 95 degC, cold 40 -> 110 degC, over a 20 -> 220 degC display
    /// range) and require: each stream's colour at path fraction 0 and 1 to
    /// equal the mapped inlet and outlet temperatures exactly; the mid-path
    /// colour to be the mapped mean; every colour to differ from
    /// [`UNKNOWN_FLUID`]; and — the arrangement-specific part — the colour drawn
    /// at the **left edge of the body** for the cold stream to be its outlet
    /// colour in counter-flow and its inlet colour in parallel flow.
    ///
    /// **Result (2026-08-12):** hot stream rgb(183, 90, 38) at its 180 degC
    /// inlet grading to rgb(147, 189, 210) at its 95 degC outlet; cold stream
    /// rgb(2, 58, 123) at its 40 degC inlet grading to rgb(206, 224, 232) at
    /// its 110 degC outlet; both mid-path colours equalled the mapped arithmetic
    /// means exactly; at the left edge, counter-flow drew the cold stream in its
    /// outlet colour rgb(206, 224, 232) and parallel flow in its inlet colour
    /// rgb(2, 58, 123). Interpretation: both streams are drawn from real
    /// supplied state, the hot stream visibly crosses the neutral midpoint of
    /// the scale as it cools, and the arrangement really does change which end
    /// of the cold stream you see where.
    #[test]
    fn the_scalar_path_colours_both_streams_from_supplied_state() {
        let r = range();
        let s = scalars();
        let drawn = visual(HeatExchangerKind::CounterFlow).state.resolve();

        for (hot, inlet, outlet) in [
            (true, s.hot_inlet_temp, s.hot_outlet_temp),
            (false, s.cold_inlet_temp, s.cold_outlet_temp),
        ] {
            let at_inlet = drawn.stream_colour(hot, 0.0);
            let at_outlet = drawn.stream_colour(hot, 1.0);
            println!(
                "{} stream {at_inlet:?} -> {at_outlet:?}",
                if hot { "hot" } else { "cold" }
            );
            assert_eq!(
                at_inlet,
                temperature_colour(inlet, r.min_temp, r.max_temp),
                "the inlet must be the caller's inlet temperature"
            );
            assert_eq!(
                at_outlet,
                temperature_colour(outlet, r.min_temp, r.max_temp),
                "the outlet must be the caller's outlet temperature"
            );
            assert_ne!(
                at_inlet, at_outlet,
                "the stream must visibly change temperature"
            );
            assert_ne!(at_inlet, UNKNOWN_FLUID);
            let mean = ThermodynamicTemperature::new::<kelvin>(
                0.5 * (inlet.get::<kelvin>() + outlet.get::<kelvin>()),
            );
            assert_eq!(
                drawn.stream_colour(hot, 0.5),
                temperature_colour(mean, r.min_temp, r.max_temp)
            );
        }

        // The arrangement decides which end of the cold stream is at the left.
        let (_, cold_left_cf) = path_fractions(HeatExchangerKind::CounterFlow, 0.0);
        let (_, cold_left_pf) = path_fractions(HeatExchangerKind::ParallelFlow, 0.0);
        let cf = drawn.stream_colour(false, cold_left_cf);
        let pf = drawn.stream_colour(false, cold_left_pf);
        assert_eq!(
            cf,
            drawn.stream_colour(false, 1.0),
            "counter-flow shows the cold OUTLET at the left"
        );
        assert_eq!(
            pf,
            drawn.stream_colour(false, 0.0),
            "parallel flow shows the cold INLET at the left"
        );
        assert_ne!(cf, pf);
    }

    /// The verdict and approaches exposed by the widget must agree with the
    /// free functions the caller can call directly, or a studio readout could
    /// disagree with the drawing beside it.
    #[test]
    fn the_widget_accessors_agree_with_the_free_functions() {
        let s = scalars();
        for kind in HeatExchangerKind::ALL {
            let v = visual(*kind);
            assert_eq!(
                v.approaches(),
                Some(terminal_approaches(
                    *kind,
                    s.hot_inlet_temp,
                    s.hot_outlet_temp,
                    s.cold_inlet_temp,
                    s.cold_outlet_temp
                ))
            );
            assert_eq!(
                v.verdict(),
                Some(approach_verdict(
                    *kind,
                    s.hot_inlet_temp,
                    s.hot_outlet_temp,
                    s.cold_inlet_temp,
                    s.cold_outlet_temp
                ))
            );
        }
        // The illustrative point is a genuine cross in counter-flow, and
        // unreachable in parallel flow.
        assert_eq!(
            visual(HeatExchangerKind::CounterFlow).verdict(),
            Some(ApproachVerdict::TemperatureCross)
        );
        assert_eq!(
            visual(HeatExchangerKind::ParallelFlow).verdict(),
            Some(ApproachVerdict::Impossible)
        );
    }

    /// The temperature gradient along a stream must interpolate in kelvin and
    /// hit its endpoints exactly, so a run's ends read as the two temperatures
    /// the caller supplied and nothing else.
    #[test]
    fn the_stream_gradient_interpolates_between_its_endpoints() {
        let (from, to) = (degc(180.0), degc(95.0));
        assert!(
            (lerp_temperature(from, to, 0.0).get::<kelvin>() - from.get::<kelvin>()).abs() < 1e-9
        );
        assert!(
            (lerp_temperature(from, to, 1.0).get::<kelvin>() - to.get::<kelvin>()).abs() < 1e-9
        );
        let mid = lerp_temperature(from, to, 0.5).get::<kelvin>();
        assert!((mid - 0.5 * (from.get::<kelvin>() + to.get::<kelvin>())).abs() < 1e-9);
        // Out-of-range positions clamp rather than extrapolating past the ends.
        assert_eq!(
            lerp_temperature(from, to, -3.0).get::<kelvin>(),
            from.get::<kelvin>()
        );
        assert_eq!(
            lerp_temperature(from, to, 9.0).get::<kelvin>(),
            to.get::<kelvin>()
        );
    }

    /// Every kind and construction must name itself, where it is used and how
    /// the streams are routed, so a gallery caption can be built without a
    /// lookup table elsewhere going stale.
    #[test]
    fn every_variant_describes_itself() {
        assert_eq!(HeatExchangerKind::ALL.len(), 2);
        assert_eq!(HeatExchangerConstruction::ALL.len(), 2);
        for kind in HeatExchangerKind::ALL {
            assert!(!kind.label().is_empty());
            assert!(!kind.description().is_empty());
            assert!(!kind.cold_stream_path().is_empty());
        }
        for construction in HeatExchangerConstruction::ALL {
            assert!(!construction.label().is_empty());
            assert!(!construction.description().is_empty());
            assert!(!construction.hot_stream_location().is_empty());
        }
        assert!(HeatExchangerKind::CounterFlow
            .description()
            .contains("opposite ends"));
        assert!(HeatExchangerKind::ParallelFlow
            .description()
            .contains("same end"));
    }

    /// Corner radii are `u8` in egui, so the helper must saturate rather than
    /// wrap — a wrapped radius would round the shell to nothing.
    #[test]
    fn corner_radii_saturate_instead_of_wrapping() {
        assert_eq!(radius(0.0), 0);
        assert_eq!(radius(12.4), 12);
        assert_eq!(radius(12.6), 13);
        assert_eq!(radius(4000.0), 255);
        assert_eq!(radius(-8.0), 0);
        assert_eq!(radius(f32::NAN), 0);
    }
}
