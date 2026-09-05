//! Schematic steam-generator art, one architecture per steam-generator type.
//!
//! The three steam generators this workspace's reactors actually use do not
//! share a shape, and the differences are the physics rather than the styling:
//!
//! - A **vertical U-tube** generator (the Western PWR standard) is a
//!   *recirculating* machine. Only steam leaves the top, so it needs moisture
//!   separators and dryers above the bundle, a water level to separate at, and
//!   a downcomer annulus to return the separated water — with the feedwater
//!   mixed in — to the bottom of the bundle.
//! - A **horizontal U-tube** generator (the VVER standard) does the same job
//!   lying down. The bundle runs horizontally between two vertical collectors,
//!   and because the free surface now runs the whole length of the vessel the
//!   steam space is broad and shallow instead of tall — which is exactly why
//!   it needs no separator stack.
//! - A **helical coil** generator (HTGR and integral-PWR practice) is
//!   *once-through*. Feedwater enters one end and superheated steam leaves the
//!   other, so there is no recirculation, no separator, no downcomer, and **no
//!   water level at all** — see [`SteamGeneratorKind::has_water_level`].
//!
//! Drawing all three as the same coloured rectangle would hide where the phase
//! change happens and how the water gets back to the bundle, which is the only
//! interesting thing about a steam generator.
//!
//! # Dispatch
//!
//! [`SteamGeneratorKind`] is an enum, not a trait object, per the workspace
//! rule: the set of architectures is closed and known at compile time, so
//! adding one is a variant and the compiler then points at every match that
//! needs handling. The same applies to [`SteamGeneratorVisualState`], which
//! chooses between caller-supplied scalars and a live
//! [`tampines::components::SteamGenerator`].
//!
//! # Provenance of the proportions
//!
//! Each variant carries its own [`SteamGeneratorKind::native_aspect_ratio`]
//! and letterboxes to it (see [`SteamGeneratorKind::fit_native_aspect`]), so
//! the artwork keeps its real slenderness at any box size:
//!
//! | Variant | Ratio (w : h) | Where the numbers come from |
//! |---|---|---|
//! | [`SteamGeneratorKind::VerticalUTube`] | 4.5 m / 20.6 m | the widely published envelope of a large Western PWR recirculating steam generator |
//! | [`SteamGeneratorKind::HorizontalUTube`] | 13.84 m / 4.0 m | the published PGV-1000M horizontal steam-generator vessel length and inner diameter |
//! | [`SteamGeneratorKind::HelicalCoil`] | 2.5 m / 11.3 m | the HTR-10 steam-generator pressure vessel, "approximately 11.3 m in height, 2.5 m inner diameter" |
//!
//! The helical-coil *arrangement* also follows the HTR-10 description in the
//! IAEA coordinated-research-programme report ingested into this workspace's
//! literature layer at
//! `crates/kovan-literature/generated/markdown/open/htr-10-iaea.md`: a
//! once-through modular helical tube type, hot helium arriving through a
//! **centre tube** to the top of the unit and then flowing **down** around the
//! tubes (cooling 700 degC to 250 degC) before returning up along the vessel
//! wall to the blower, with the water flowing through the helical tubes
//! **from the bottom to the top** (feedwater 104 degC, steam 435 degC at the
//! turbine inlet).
//!
//! # What this is not
//!
//! **This is offline demonstration artwork, not a validated model and not a
//! design drawing.** Only the overall envelope ratios above are taken from
//! published dimensions; every internal feature is proportioned by eye for
//! legibility, and nothing here represents a specific licensed design. The
//! drawn temperature gradients are display interpolations between the scalars
//! the caller supplies — they are not the output of a heat balance, and this
//! crate deliberately owns no physics (see the crate `CLAUDE.md`). See
//! `RESPONSIBLE_USE.md`.

use crate::components::temperature_colour;
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Shape};
use egui::{Stroke, StrokeKind, Ui, Vec2, Widget};
use std::f32::consts::PI;
use tampines::components::SteamGenerator;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

// ── Envelope proportions ────────────────────────────────────────────────────

/// Width-to-height ratio of the vertical U-tube generator, dimensionless.
///
/// A large Western PWR recirculating steam generator stands roughly 20.6 m
/// high over an upper-shell diameter of about 4.5 m, so the silhouette is very
/// slender — which is what makes room for the separator stack above the
/// bundle.
pub const VERTICAL_U_TUBE_ASPECT_RATIO: f32 = 4.5 / 20.6;

/// Width-to-height ratio of the horizontal U-tube (VVER) generator,
/// dimensionless.
///
/// From the published PGV-1000M vessel: about 13.84 m long over a 4.0 m inner
/// diameter. This is the only variant wider than it is tall, and the reason is
/// physical — the free surface has to run the length of the vessel.
pub const HORIZONTAL_U_TUBE_ASPECT_RATIO: f32 = 13.84 / 4.0;

/// Width-to-height ratio of the helical-coil generator, dimensionless.
///
/// From the HTR-10 steam-generator pressure vessel: approximately 11.3 m high
/// by 2.5 m inner diameter. That vessel also houses the helium blower above
/// the tube bundle; only the bundle's own envelope is drawn here.
pub const HELICAL_COIL_ASPECT_RATIO: f32 = 2.5 / 11.3;

/// Which steam-generator architecture to draw.
///
/// Each variant is a genuinely different machine, not a restyling: see the
/// module documentation for how recirculating and once-through units differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamGeneratorKind {
    /// Vertical recirculating U-tube generator — the Western PWR standard.
    ///
    /// Inverted-U bundle in the lower shell, moisture separators and dryers
    /// above it, a downcomer annulus outside the tube wrapper, and a
    /// hemispherical channel head split by a divider plate into hot-leg and
    /// cold-leg chambers.
    VerticalUTube,
    /// Horizontal recirculating U-tube generator — the VVER standard.
    ///
    /// Horizontal cylindrical vessel; primary water rises and falls through
    /// two vertical cylindrical collectors, with the tube bundle running
    /// horizontally between them under a water level that spans the vessel.
    HorizontalUTube,
    /// Once-through helical-coil generator — HTGR and integral-PWR practice.
    ///
    /// Helical bundle wound around a central column, counter-current, with no
    /// separators, no downcomer and no water level.
    HelicalCoil,
}

impl SteamGeneratorKind {
    /// Every architecture, in the order a gallery should show them.
    pub const ALL: &'static [Self] = &[
        Self::VerticalUTube,
        Self::HorizontalUTube,
        Self::HelicalCoil,
    ];

    /// Short display name, for a picker or a card caption.
    pub fn label(self) -> &'static str {
        match self {
            Self::VerticalUTube => "Vertical U-tube",
            Self::HorizontalUTube => "Horizontal U-tube",
            Self::HelicalCoil => "Helical coil",
        }
    }

    /// Which reactor family uses this architecture, in words.
    pub fn description(self) -> &'static str {
        match self {
            Self::VerticalUTube => "Western PWR standard",
            Self::HorizontalUTube => "VVER standard",
            Self::HelicalCoil => "HTGR / integral PWR",
        }
    }

    /// How the secondary side is circulated — the single fact that explains
    /// every geometric difference between the variants.
    pub fn circulation(self) -> &'static str {
        match self {
            Self::VerticalUTube => "recirculating — only steam leaves the top",
            Self::HorizontalUTube => "recirculating — steam leaves along the top",
            Self::HelicalCoil => "once-through — feedwater in, superheated steam out",
        }
    }

    /// Whether this architecture has a secondary-side water level at all.
    ///
    /// `false` for [`Self::HelicalCoil`]: a once-through generator boils the
    /// whole feedwater flow to superheat inside the tubes, so there is no free
    /// surface to hold a level on. A caller may still pass a level fraction —
    /// it is simply ignored, and [`drawn_water_level`] returns `None`.
    pub fn has_water_level(self) -> bool {
        match self {
            Self::VerticalUTube | Self::HorizontalUTube => true,
            Self::HelicalCoil => false,
        }
    }

    /// Width-to-height ratio the artwork is drawn at, dimensionless.
    ///
    /// See the module table for where each number comes from.
    pub fn native_aspect_ratio(self) -> f32 {
        match self {
            Self::VerticalUTube => VERTICAL_U_TUBE_ASPECT_RATIO,
            Self::HorizontalUTube => HORIZONTAL_U_TUBE_ASPECT_RATIO,
            Self::HelicalCoil => HELICAL_COIL_ASPECT_RATIO,
        }
    }

    /// The largest sub-rectangle of `available` carrying this kind's
    /// [`Self::native_aspect_ratio`], centred within it.
    ///
    /// Same letterbox contract as the reactor vessels: the artwork keeps its
    /// proportions at any box size rather than stretching to fill its box, so
    /// a wide VVER generator stays wide in a tall card and a vertical PWR
    /// generator stays slender in a square one. A degenerate box (zero or
    /// negative extent, as egui layout can transiently produce) is returned
    /// unchanged rather than producing NaN geometry.
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

/// The secondary-side water level actually drawn, dimensionless in `[0, 1]`.
///
/// `0.0` is a drained generator (level at the tubesheet, or at the bottom of a
/// horizontal vessel) and `1.0` is a full one (level at the separator inlet,
/// or at the top of a horizontal vessel). Normal operation sits well inside
/// that range.
///
/// Returns `None` for [`SteamGeneratorKind::HelicalCoil`], which has no free
/// surface to hold a level on.
///
/// Out-of-range values are clamped rather than rejected, because a level comes
/// from a controller that can transiently overshoot and artwork must not panic
/// on it. A **non-finite** level draws as empty (`0.0`) — deliberately the most
/// visible outcome, so a NaN in the caller's model shows up on screen instead
/// of hiding behind a plausible mid-drum level.
pub fn drawn_water_level(kind: SteamGeneratorKind, frac: f32) -> Option<f32> {
    if !kind.has_water_level() {
        return None;
    }
    if !frac.is_finite() {
        return Some(0.0);
    }
    Some(frac.clamp(0.0, 1.0))
}

// ── State ───────────────────────────────────────────────────────────────────

/// Scalar thermal state of a steam generator, as the caller's own model holds
/// it.
///
/// All four temperatures are absolute thermodynamic temperatures (`uom`-typed,
/// so the compiler enforces the unit; they are kelvin internally and are
/// conventionally quoted in degrees Celsius). They are used only for colour,
/// via the shared [`crate::components::temperature_colour`] map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteamGeneratorScalars {
    /// Primary coolant entering the generator — the hot leg. Drawn on the
    /// inlet plenum/collector and at the start of the tube gradient.
    pub primary_inlet_temp: ThermodynamicTemperature,
    /// Primary coolant leaving the generator — the cold leg. Drawn on the
    /// outlet plenum/collector and at the end of the tube gradient.
    pub primary_outlet_temp: ThermodynamicTemperature,
    /// Secondary feedwater entering the generator. Drawn on the feedwater
    /// nozzle, the distribution ring/header and the downcomer.
    pub feedwater_temp: ThermodynamicTemperature,
    /// Secondary steam leaving the generator. Drawn on the steam space, the
    /// steam nozzle, and — for a recirculating unit, whose bulk water sits at
    /// saturation — on the water region too.
    pub steam_temp: ThermodynamicTemperature,
    /// Secondary water level, dimensionless in `[0, 1]`; see
    /// [`drawn_water_level`]. Ignored by
    /// [`SteamGeneratorKind::HelicalCoil`].
    pub water_level_frac: f32,
}

/// A [`tampines::components::SteamGenerator`] plus the scalars it does not
/// itself expose.
///
/// The physics component owns a secondary-side
/// [`tampines::hem::HemSteamCv`], whose `get_temperature()` is a real working
/// getter — that is where [`SteamGeneratorScalars::steam_temp`] comes from on
/// this path, so the steam space and the steam nozzle track the real state.
///
/// It exposes **no primary-side temperature** (its `step` is not yet
/// implemented), so the primary inlet/outlet, the feedwater temperature and
/// the level are supplied alongside it by the caller. They are not fabricated:
/// [`SteamGeneratorVisual::new`] starts them all at the secondary temperature,
/// which draws an isothermal generator until a caller supplies real values
/// with [`SteamGeneratorVisual::with_primary_temperatures`],
/// [`SteamGeneratorVisual::with_feedwater_temperature`] and
/// [`SteamGeneratorVisual::with_water_level`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteamGeneratorPhysicsState {
    /// The underlying TAMPINES component.
    pub physics: SteamGenerator,
    /// Primary coolant entering the generator (hot leg).
    pub primary_inlet_temp: ThermodynamicTemperature,
    /// Primary coolant leaving the generator (cold leg).
    pub primary_outlet_temp: ThermodynamicTemperature,
    /// Secondary feedwater entering the generator.
    pub feedwater_temp: ThermodynamicTemperature,
    /// Secondary water level, dimensionless in `[0, 1]`.
    pub water_level_frac: f32,
}

/// Where a [`SteamGeneratorVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SteamGeneratorVisualState {
    /// Backed by caller-supplied scalars — a simulator that already holds
    /// these temperatures in its own plant model.
    Scalars(SteamGeneratorScalars),
    /// Backed by a live TAMPINES steam generator; the steam temperature is
    /// read from its secondary-side control volume every frame.
    Physics(SteamGeneratorPhysicsState),
}

impl SteamGeneratorVisualState {
    /// The scalars the artwork is actually drawn from.
    ///
    /// For [`Self::Physics`] this reads
    /// [`tampines::hem::HemSteamCv::get_temperature`] on the secondary side,
    /// so the physics path is live data rather than a snapshot taken at
    /// construction.
    pub fn resolve(&self) -> SteamGeneratorScalars {
        match self {
            Self::Scalars(s) => *s,
            Self::Physics(p) => SteamGeneratorScalars {
                primary_inlet_temp: p.primary_inlet_temp,
                primary_outlet_temp: p.primary_outlet_temp,
                feedwater_temp: p.feedwater_temp,
                steam_temp: p.physics.secondary_side.get_temperature(),
                water_level_frac: p.water_level_frac,
            },
        }
    }
}

// ── Deterministic scatter ───────────────────────────────────────────────────

/// Deterministic pseudo-random value in `[0, 1)` from two indices.
///
/// **Determinism is the point.** Widgets here are consumed by value and
/// rebuilt on every repaint, so drawing droplets or tube sag from a real
/// random source would make them shimmer — every feature jumping to a new spot
/// each frame. Hashing the indices instead gives a scatter that looks random
/// but is identical frame to frame.
///
/// `salt` separates the independent draws (x offset, y offset, radius) so they
/// do not correlate into visible stripes. This is the same construction as the
/// pebble-bed packing hash in
/// [`crate::components::htr10_reactor_vessel`], with its own salts so the two
/// never share a pattern.
fn sg_hash(a: i32, b: i32, salt: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9)
        ^ (b as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h % 1_000_003) as f32 / 1_000_003.0
}

/// Deterministic scatter point number `index` inside `area`.
///
/// Used for entrained droplets in a steam space and for bubbles rising through
/// the water. `salt` picks an independent scatter, so two features drawn in
/// overlapping regions do not land on top of each other.
///
/// The point is always inside `area` (inclusive of its edges) provided `area`
/// is non-degenerate, and is a pure function of `(index, salt)` — see
/// [`sg_hash`] for why that matters.
fn scatter_point(area: Rect, index: i32, salt: u32) -> Pos2 {
    Pos2::new(
        area.left() + sg_hash(index, 0, salt) * area.width(),
        area.top() + sg_hash(index, 1, salt.wrapping_add(1)) * area.height(),
    )
}

// ── Geometry helpers ────────────────────────────────────────────────────────

/// Half-separation of the legs of U-tube number `index` of `count`, as a
/// distance in screen points from the bundle centreline.
///
/// Real bundles nest their tubes: every tube's U-bend radius equals half its
/// own leg separation, so the outermost tube reaches highest and the bundle
/// acquires its characteristic domed top. Drawing the offsets on a uniform
/// ladder inside `bundle_half_width` reproduces that for free — the apex of
/// tube `index` is simply its offset above the bend centreline.
///
/// Strictly increasing in `index`, and always less than `bundle_half_width` so
/// no tube escapes the wrapper.
fn u_tube_leg_offset(index: usize, count: usize, bundle_half_width: f32) -> f32 {
    let denominator = count as f32 + 0.6;
    bundle_half_width * (index as f32 + 1.0) / denominator
}

/// Linear interpolation between two temperatures, in kelvin.
///
/// `t` is a dimensionless position along whatever path is being coloured,
/// clamped to `[0, 1]`. **This is a display interpolation, not physics**: a
/// real tube's temperature profile is set by a heat balance (and, in a boiling
/// or condensing region, is nearly flat at saturation). Interpolating for
/// colour is honest as artwork and is stated as such everywhere it is used.
fn lerp_temperature(
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

/// Pressure-boundary steel.
const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
/// Tubesheet and other heavy forgings, a shade lighter so they read as
/// separate parts rather than merging into the shell.
const FORGING: Color32 = Color32::from_rgb(126, 131, 140);
/// Vessel outline, drawn last so the silhouette reads on top.
const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
/// Internals that carry no interesting temperature: wrappers, support plates,
/// separator cans, dryer vanes, divider plates.
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
/// Unfilled interior, behind the coloured regions.
const VOID: Color32 = Color32::from_rgb(28, 30, 34);
/// Label text.
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);

// ── The widget ──────────────────────────────────────────────────────────────

/// Visual representation of a steam generator, in one of three architectures.
///
/// Scalar-fed like the reactor vessels, and for the same reason: a simulator
/// already holds these temperatures in its own plant model, and standing up a
/// whole secondary-side control volume to colour a schematic would be
/// disproportionate. The physics-backed path is still available and still
/// real — see [`Self::new`] and [`SteamGeneratorVisualState::Physics`].
///
/// All temperatures are absolute thermodynamic temperatures (`uom`-typed).
/// `min_temp`/`max_temp` bound the diverging colour scale; because the map is
/// diverging (blue at min, neutral white at the *midpoint*, red at max), set
/// them about a reference that matters rather than clamping to the extremes
/// seen.
pub struct SteamGeneratorVisual {
    kind: SteamGeneratorKind,
    state: SteamGeneratorVisualState,
    screen_position: Pos2,
    screen_vector: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    show_labels: bool,
}

impl SteamGeneratorVisual {
    /// Wrap a [`SteamGenerator`] with the given screen geometry and
    /// colour-mapping temperature range.
    ///
    /// This is the **physics-backed** path, and it is unchanged from the
    /// original widget: the secondary side is coloured by
    /// [`tampines::hem::HemSteamCv::get_temperature`], read live every repaint.
    ///
    /// The component exposes nothing about the primary side, so the primary
    /// inlet/outlet and feedwater temperatures start equal to the secondary
    /// temperature — an isothermal generator, which is visibly uninformative
    /// rather than quietly wrong — and the level starts at 0.5. Supply real
    /// values with [`Self::with_primary_temperatures`],
    /// [`Self::with_feedwater_temperature`] and [`Self::with_water_level`].
    ///
    /// Defaults to [`SteamGeneratorKind::VerticalUTube`]; change it with
    /// [`Self::with_kind`].
    ///
    /// `screen_position` is the **centre** of the box the artwork is
    /// letterboxed into, and `screen_vector` its size in points.
    pub fn new(
        physics: SteamGenerator,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        let secondary = physics.secondary_side.get_temperature();
        Self {
            kind: SteamGeneratorKind::VerticalUTube,
            state: SteamGeneratorVisualState::Physics(SteamGeneratorPhysicsState {
                physics,
                primary_inlet_temp: secondary,
                primary_outlet_temp: secondary,
                feedwater_temp: secondary,
                water_level_frac: 0.5,
            }),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            show_labels: true,
        }
    }

    /// Build a generator visual from the caller's own scalar plant state.
    ///
    /// `screen_position` is the centre of the box, `screen_vector` its size in
    /// points; the artwork letterboxes to `kind`'s native proportions inside
    /// it.
    pub fn from_scalars(
        kind: SteamGeneratorKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        scalars: SteamGeneratorScalars,
    ) -> Self {
        Self {
            kind,
            state: SteamGeneratorVisualState::Scalars(scalars),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            show_labels: true,
        }
    }

    /// Draw a different architecture with the same state.
    pub fn with_kind(mut self, kind: SteamGeneratorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the primary hot-leg and cold-leg temperatures.
    pub fn with_primary_temperatures(
        mut self,
        inlet: ThermodynamicTemperature,
        outlet: ThermodynamicTemperature,
    ) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => {
                s.primary_inlet_temp = inlet;
                s.primary_outlet_temp = outlet;
            }
            SteamGeneratorVisualState::Physics(p) => {
                p.primary_inlet_temp = inlet;
                p.primary_outlet_temp = outlet;
            }
        }
        self
    }

    /// Set the secondary feedwater temperature.
    pub fn with_feedwater_temperature(mut self, feedwater: ThermodynamicTemperature) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => s.feedwater_temp = feedwater,
            SteamGeneratorVisualState::Physics(p) => p.feedwater_temp = feedwater,
        }
        self
    }

    /// Set the secondary water level, dimensionless in `[0, 1]`.
    ///
    /// Stored as given and clamped at render time (see [`drawn_water_level`]),
    /// matching the reactor vessels' contract for control-rod insertion.
    /// Ignored by [`SteamGeneratorKind::HelicalCoil`], which has no level.
    pub fn with_water_level(mut self, frac: f32) -> Self {
        match &mut self.state {
            SteamGeneratorVisualState::Scalars(s) => s.water_level_frac = frac,
            SteamGeneratorVisualState::Physics(p) => p.water_level_frac = frac,
        }
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Which architecture this visual draws.
    pub fn kind(&self) -> SteamGeneratorKind {
        self.kind
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.screen_vector
    }

    /// The scalars the artwork is drawn from, resolving the physics path if
    /// that is what is wired up.
    pub fn scalars(&self) -> SteamGeneratorScalars {
        self.state.resolve()
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
            FontId::proportional(8.5),
            LABEL,
        );
    }

    /// Draws a polyline whose colour grades from `from` at its start to `to`
    /// at its end, by arc length.
    ///
    /// Used for tube bundles: a U-tube carries primary water in at one leg and
    /// out at the other, so a single flat colour would hide the only thing the
    /// tube is there to do. The gradient is a display interpolation (see
    /// [`lerp_temperature`]), not a computed profile.
    fn graded_path(
        &self,
        painter: &Painter,
        points: &[Pos2],
        from: ThermodynamicTemperature,
        to: ThermodynamicTemperature,
        width: f32,
    ) {
        if points.len() < 2 {
            return;
        }
        let mut cumulative = Vec::with_capacity(points.len());
        let mut total = 0.0;
        cumulative.push(0.0);
        for pair in points.windows(2) {
            total += (pair[1] - pair[0]).length();
            cumulative.push(total);
        }
        if !(total > 0.0) {
            return;
        }
        for (i, pair) in points.windows(2).enumerate() {
            let fraction = 0.5 * (cumulative[i] + cumulative[i + 1]) / total;
            let colour = self.colour(lerp_temperature(from, to, fraction));
            painter.line_segment([pair[0], pair[1]], Stroke::new(width, colour));
        }
    }
}

impl Widget for SteamGeneratorVisual {
    /// Draws the steam generator for [`SteamGeneratorVisual::kind`], coloured
    /// by the four supplied temperatures via the shared
    /// [`crate::components::temperature_colour`] map, so this widget grades
    /// temperature identically to every other component in the library.
    ///
    /// The artwork letterboxes to the kind's native proportions inside the
    /// allocated box (see [`SteamGeneratorKind::fit_native_aspect`]), so it
    /// never stretches.
    fn ui(self, ui: &mut Ui) -> Response {
        let box_rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(box_rect, Sense::hover());
        let painter = ui.painter_at(box_rect);

        let rect = self.kind.fit_native_aspect(box_rect);
        let scalars = self.state.resolve();

        match self.kind {
            SteamGeneratorKind::VerticalUTube => self.draw_vertical_u_tube(&painter, rect, scalars),
            SteamGeneratorKind::HorizontalUTube => {
                self.draw_horizontal_u_tube(&painter, rect, scalars)
            }
            SteamGeneratorKind::HelicalCoil => self.draw_helical_coil(&painter, rect, scalars),
        }

        response
    }
}

// ── 1. Vertical U-tube (Western PWR) ────────────────────────────────────────

impl SteamGeneratorVisual {
    /// Draws the vertical recirculating U-tube generator.
    ///
    /// Bottom to top: a hemispherical channel head split by a divider plate
    /// into a hot-leg inlet chamber and a cold-leg outlet chamber, each with
    /// its own primary nozzle; the tubesheet; the inverted-U tube bundle
    /// inside its wrapper, with support plates and the U-bends forming the
    /// domed top of the bundle; the downcomer annulus outside the wrapper
    /// carrying feedwater down from the distribution ring; the water level and
    /// the steam space above it; the swirl-vane moisture separators; the
    /// chevron dryers; and the steam outlet nozzle in the head.
    ///
    /// Illustrative geometry — the proportions of the internals are chosen for
    /// legibility, not dimensioned from any design.
    fn draw_vertical_u_tube(&self, painter: &Painter, rect: Rect, s: SteamGeneratorScalars) {
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let y = |f: f32| rect.top() + f * h;

        let upper_half = w * 0.5;
        let lower_half = w * 0.378;

        let dryer_top = y(0.07);
        let dryer_bottom = y(0.19);
        let separator_bottom = y(0.30);
        let cone_top = y(0.30);
        let cone_bottom = y(0.38);
        let tubesheet_top = y(0.87);
        let tubesheet_bottom = y(0.90);

        let primary_in = self.colour(s.primary_inlet_temp);
        let primary_out = self.colour(s.primary_outlet_temp);
        let feed = self.colour(s.feedwater_temp);
        let steam = self.colour(s.steam_temp);

        // ── Pressure boundary: domed head, upper shell, transition cone,
        //    lower shell ──────────────────────────────────────────────────
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - upper_half, rect.top()),
                Pos2::new(cx + upper_half, y(0.10)),
            ),
            radius(upper_half * 0.7),
            STEEL,
        );
        let upper_shell = Rect::from_min_max(
            Pos2::new(cx - upper_half, y(0.05)),
            Pos2::new(cx + upper_half, cone_top),
        );
        painter.rect_filled(upper_shell, 0.0, STEEL);
        painter.add(Shape::convex_polygon(
            vec![
                Pos2::new(cx - upper_half, cone_top),
                Pos2::new(cx + upper_half, cone_top),
                Pos2::new(cx + lower_half, cone_bottom),
                Pos2::new(cx - lower_half, cone_bottom),
            ],
            STEEL,
            Stroke::NONE,
        ));
        let lower_shell = Rect::from_min_max(
            Pos2::new(cx - lower_half, cone_bottom),
            Pos2::new(cx + lower_half, tubesheet_bottom),
        );
        painter.rect_filled(lower_shell, 0.0, STEEL);

        // ── Secondary interior: wrapper, water, steam ──────────────────────
        let wrapper_half = lower_half - w * 0.085;
        let interior = Rect::from_min_max(
            Pos2::new(cx - wrapper_half, y(0.10)),
            Pos2::new(cx + wrapper_half, tubesheet_top),
        );
        painter.rect_filled(interior, 0.0, VOID);

        // The level runs from the tubesheet (empty) to the separator inlet
        // (full).
        let level =
            drawn_water_level(SteamGeneratorKind::VerticalUTube, s.water_level_frac).unwrap_or(0.0);
        let level_y = tubesheet_top - level * (tubesheet_top - cone_top);

        // Bulk water in a recirculating generator sits at saturation, which is
        // the temperature the steam leaves at — so it is drawn at the steam
        // temperature rather than at the feedwater temperature.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - wrapper_half, level_y),
                Pos2::new(cx + wrapper_half, tubesheet_top),
            ),
            0.0,
            translucent(steam, 90),
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - wrapper_half, y(0.10)),
                Pos2::new(cx + wrapper_half, level_y),
            ),
            0.0,
            translucent(steam, 40),
        );

        // Entrained droplets carried up into the steam space, deterministic so
        // they do not shimmer between repaints.
        if level_y > separator_bottom + h * 0.01 {
            let mist = Rect::from_min_max(
                Pos2::new(cx - wrapper_half * 0.9, separator_bottom + h * 0.005),
                Pos2::new(cx + wrapper_half * 0.9, level_y - h * 0.005),
            );
            for i in 0..16 {
                let p = scatter_point(mist, i, 31);
                painter.circle_filled(p, (w * 0.012).max(0.8), translucent(steam, 170));
            }
        }

        // ── Downcomer annulus, fed by the feedwater ring ───────────────────
        for side in [-1.0f32, 1.0] {
            let outer = cx + side * (lower_half - w * 0.012);
            let inner = cx + side * wrapper_half;
            let (left, right) = if side < 0.0 {
                (outer, inner)
            } else {
                (inner, outer)
            };
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(left, cone_bottom + h * 0.005),
                    Pos2::new(right, tubesheet_top),
                ),
                0.0,
                translucent(feed, 205),
            );
            // Feedwater ring segment, discharging into the top of the annulus.
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(left, y(0.395)), Pos2::new(right, y(0.415))),
                2.0,
                feed,
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.435)), "downcomer");

        // Feedwater nozzle on the upper shell, just above the transition.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx + upper_half - w * 0.02, y(0.275)),
                Pos2::new(cx + upper_half + w * 0.20, y(0.30)),
            ),
            2.0,
            feed,
        );

        // ── Tube bundle: nested inverted U-tubes ───────────────────────────
        let bundle_half = wrapper_half - w * 0.035;
        let bend_centre = y(0.47);
        let tube_count = 7usize;
        let tube_width = (w * 0.016).max(1.0);
        for i in 0..tube_count {
            let offset = u_tube_leg_offset(i, tube_count, bundle_half);
            let mut path = vec![
                Pos2::new(cx - offset, tubesheet_top),
                Pos2::new(cx - offset, bend_centre),
            ];
            // Semicircular U-bend of radius `offset`, so the outer tubes reach
            // highest and the bundle acquires its domed top.
            let steps = 14;
            for k in 1..steps {
                let angle = PI - PI * (k as f32) / (steps as f32);
                path.push(Pos2::new(
                    cx + offset * angle.cos(),
                    bend_centre - offset * angle.sin(),
                ));
            }
            path.push(Pos2::new(cx + offset, bend_centre));
            path.push(Pos2::new(cx + offset, tubesheet_top));
            self.graded_path(
                painter,
                &path,
                s.primary_inlet_temp,
                s.primary_outlet_temp,
                tube_width,
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.55)), "U-tube bundle");

        // Tube support plates.
        for f in [0.56f32, 0.65, 0.74, 0.83] {
            painter.line_segment(
                [
                    Pos2::new(cx - bundle_half - w * 0.01, y(f)),
                    Pos2::new(cx + bundle_half + w * 0.01, y(f)),
                ],
                Stroke::new((h * 0.004).max(1.0), INTERNALS),
            );
        }

        // Tube wrapper (shroud) separating the bundle from the downcomer.
        for side in [-1.0f32, 1.0] {
            let x = cx + side * wrapper_half;
            painter.line_segment(
                [Pos2::new(x, cone_bottom), Pos2::new(x, tubesheet_top)],
                Stroke::new((w * 0.012).max(1.0), INTERNALS),
            );
        }

        // ── Water level ────────────────────────────────────────────────────
        let mut wave = Vec::with_capacity(21);
        for k in 0..=20 {
            let t = k as f32 / 20.0;
            let x = cx - wrapper_half + t * 2.0 * wrapper_half;
            wave.push(Pos2::new(x, level_y + (t * PI * 3.0).sin() * h * 0.002));
        }
        for pair in wave.windows(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(1.6_f32, steam));
        }
        self.tag(painter, Pos2::new(cx, level_y - h * 0.016), "level");

        // ── Moisture separators, then chevron dryers above them ────────────
        for k in -2..=2 {
            let x = cx + k as f32 * upper_half * 0.34;
            let can = Rect::from_min_max(
                Pos2::new(x - w * 0.06, y(0.195)),
                Pos2::new(x + w * 0.06, separator_bottom),
            );
            painter.rect_filled(can, radius(w * 0.02), INTERNALS);
            // Swirl vanes.
            for v in 0..3 {
                let t = 0.2 + 0.3 * v as f32;
                painter.line_segment(
                    [
                        Pos2::new(can.left(), can.top() + t * can.height()),
                        Pos2::new(can.right(), can.top() + (t + 0.16) * can.height()),
                    ],
                    Stroke::new(1.2_f32, translucent(steam, 190)),
                );
            }
        }
        self.tag(painter, Pos2::new(cx, y(0.245)), "separators");

        for row in 0..3 {
            let y_top = dryer_top + (dryer_bottom - dryer_top) * (row as f32 + 0.15) / 3.0;
            let y_bottom = dryer_top + (dryer_bottom - dryer_top) * (row as f32 + 0.85) / 3.0;
            for c in 0..7 {
                let x0 = cx - upper_half * 0.82 + c as f32 * upper_half * 0.235;
                let x1 = x0 + upper_half * 0.11;
                let x2 = x0 + upper_half * 0.22;
                painter.line_segment(
                    [Pos2::new(x0, y_bottom), Pos2::new(x1, y_top)],
                    Stroke::new(1.2_f32, INTERNALS),
                );
                painter.line_segment(
                    [Pos2::new(x1, y_top), Pos2::new(x2, y_bottom)],
                    Stroke::new(1.2_f32, INTERNALS),
                );
            }
        }
        self.tag(painter, Pos2::new(cx, y(0.045)), "dryers");

        // Steam outlet nozzle in the head.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - w * 0.10, rect.top() - h * 0.018),
                Pos2::new(cx + w * 0.10, y(0.035)),
            ),
            2.0,
            steam,
        );

        // ── Tubesheet and the divided channel head ─────────────────────────
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - lower_half, tubesheet_top),
                Pos2::new(cx + lower_half, tubesheet_bottom),
            ),
            0.0,
            FORGING,
        );
        self.tag(painter, Pos2::new(cx, y(0.885)), "tubesheet");

        let head_radius = (lower_half * 0.98).min(rect.bottom() - tubesheet_bottom);
        let sample = |a: f32| {
            Pos2::new(
                cx + head_radius * a.cos(),
                tubesheet_bottom + head_radius * a.sin(),
            )
        };
        for (start, end, fill) in [
            (PI * 0.5, PI, primary_in),   // left half: hot-leg inlet chamber
            (0.0, PI * 0.5, primary_out), // right half: cold-leg outlet chamber
        ] {
            let mut poly = vec![Pos2::new(cx, tubesheet_bottom)];
            for k in 0..=10 {
                poly.push(sample(start + (end - start) * k as f32 / 10.0));
            }
            painter.add(Shape::convex_polygon(poly, fill, Stroke::NONE));
        }
        // Divider plate splitting the head into the two chambers.
        painter.line_segment(
            [
                Pos2::new(cx, tubesheet_bottom),
                Pos2::new(cx, tubesheet_bottom + head_radius),
            ],
            Stroke::new((w * 0.02).max(1.5), INTERNALS),
        );
        self.tag(painter, Pos2::new(cx - lower_half * 0.5, y(0.945)), "hot");
        self.tag(painter, Pos2::new(cx + lower_half * 0.5, y(0.945)), "cold");

        // Primary nozzles, one per chamber.
        let nozzle_y = tubesheet_bottom + head_radius * 0.45;
        let nozzle_h = h * 0.022;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - lower_half - w * 0.20, nozzle_y - nozzle_h),
                Pos2::new(cx - lower_half * 0.6, nozzle_y + nozzle_h),
            ),
            2.0,
            primary_in,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx + lower_half * 0.6, nozzle_y - nozzle_h),
                Pos2::new(cx + lower_half + w * 0.20, nozzle_y + nozzle_h),
            ),
            2.0,
            primary_out,
        );

        // Silhouette last, so it reads on top.
        painter.rect_stroke(
            upper_shell,
            0.0,
            Stroke::new(1.5_f32, OUTLINE),
            StrokeKind::Middle,
        );
        painter.rect_stroke(
            lower_shell,
            0.0,
            Stroke::new(1.5_f32, OUTLINE),
            StrokeKind::Middle,
        );
    }
}

// ── 2. Horizontal U-tube (VVER) ─────────────────────────────────────────────

impl SteamGeneratorVisual {
    /// Draws the horizontal recirculating U-tube (VVER) generator.
    ///
    /// A horizontal cylindrical vessel with hemispherical ends. Primary water
    /// enters the **hot collector** — a vertical cylinder penetrating the
    /// lower shell — spreads through the horizontal tube bundle, and leaves
    /// through the **cold collector** at the other end. The water level runs
    /// the length of the vessel above the bundle, with the feedwater
    /// distribution header along the top of the bundle, a submerged perforated
    /// sheet above it, and several steam nozzles along the top of the shell.
    ///
    /// Illustrative geometry — feature positions are proportioned for
    /// legibility, not dimensioned from any design.
    fn draw_horizontal_u_tube(&self, painter: &Painter, rect: Rect, s: SteamGeneratorScalars) {
        let w = rect.width();
        let h = rect.height();

        let primary_in = self.colour(s.primary_inlet_temp);
        let primary_out = self.colour(s.primary_outlet_temp);
        let feed = self.colour(s.feedwater_temp);
        let steam = self.colour(s.steam_temp);

        // ── Pressure boundary: a capsule ───────────────────────────────────
        painter.rect_filled(rect, radius(h * 0.5), STEEL);
        let inner = rect.shrink(h * 0.035);
        painter.rect_filled(inner, radius(inner.height() * 0.5), VOID);

        // ── Water level, spanning the whole length ─────────────────────────
        let level = drawn_water_level(SteamGeneratorKind::HorizontalUTube, s.water_level_frac)
            .unwrap_or(0.0);
        let level_y = inner.bottom() - level * inner.height();
        let end_radius = radius(inner.height() * 0.5);

        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(inner.left(), level_y),
                Pos2::new(inner.right(), inner.bottom()),
            ),
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: end_radius,
                se: end_radius,
            },
            translucent(steam, 90),
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top()),
                Pos2::new(inner.right(), level_y),
            ),
            CornerRadius {
                nw: end_radius,
                ne: end_radius,
                sw: 0,
                se: 0,
            },
            translucent(steam, 40),
        );
        painter.line_segment(
            [
                Pos2::new(inner.left(), level_y),
                Pos2::new(inner.right(), level_y),
            ],
            Stroke::new(1.6_f32, steam),
        );
        self.tag(
            painter,
            Pos2::new(inner.left() + w * 0.06, level_y - h * 0.055),
            "level",
        );
        self.tag(
            painter,
            Pos2::new(inner.center().x, inner.top() + h * 0.10),
            "steam space",
        );

        // ── Collectors: vertical cylinders penetrating the lower shell ─────
        let hot_x = inner.left() + inner.width() * 0.28;
        let cold_x = inner.left() + inner.width() * 0.72;
        let collector_half = h * 0.055;
        for (x, fill, name) in [
            (hot_x, primary_in, "hot collector"),
            (cold_x, primary_out, "cold collector"),
        ] {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - collector_half, inner.top() + h * 0.06),
                    Pos2::new(x + collector_half, rect.bottom() + h * 0.12),
                ),
                radius(collector_half * 0.5),
                fill,
            );
            self.tag(painter, Pos2::new(x, inner.top() + h * 0.035), name);
        }

        // ── Horizontal tube bundle between the collectors ──────────────────
        let bundle_top = inner.center().y + h * 0.02;
        let bundle_bottom = inner.bottom() - h * 0.09;
        let rows = 9;
        let tube_width = (h * 0.012).max(0.9);
        for row in 0..rows {
            let y_row =
                bundle_top + (bundle_bottom - bundle_top) * (row as f32 + 0.5) / rows as f32;
            // A shallow, deterministic sag, so the runs read as tubes rather
            // than as a ruled grid.
            let sag = (h * 0.012) * (0.5 + sg_hash(row as i32, 0, 37));
            let x0 = hot_x + collector_half;
            let x1 = cold_x - collector_half;
            let mut path = Vec::with_capacity(17);
            for k in 0..=16 {
                let t = k as f32 / 16.0;
                path.push(Pos2::new(x0 + (x1 - x0) * t, y_row + sag * (t * PI).sin()));
            }
            self.graded_path(
                painter,
                &path,
                s.primary_inlet_temp,
                s.primary_outlet_temp,
                tube_width,
            );
        }
        self.tag(
            painter,
            Pos2::new(inner.center().x, bundle_bottom + h * 0.055),
            "horizontal tube bundle",
        );

        // Submerged perforated sheet above the bundle.
        let sheet_y = bundle_top - h * 0.06;
        let mut x = inner.left() + w * 0.06;
        while x < inner.right() - w * 0.06 {
            painter.line_segment(
                [Pos2::new(x, sheet_y), Pos2::new(x + w * 0.012, sheet_y)],
                Stroke::new(1.3_f32, INTERNALS),
            );
            x += w * 0.02;
        }

        // Bubbles rising off the bundle through the water.
        if level_y < sheet_y {
            let boiling = Rect::from_min_max(
                Pos2::new(inner.left() + w * 0.10, level_y + h * 0.02),
                Pos2::new(inner.right() - w * 0.10, sheet_y),
            );
            for i in 0..22 {
                let p = scatter_point(boiling, i, 41);
                painter.circle_filled(p, (h * 0.012).max(0.8), translucent(steam, 150));
            }
        }

        // ── Feedwater header along the top of the bundle ───────────────────
        let header_y = sheet_y - h * 0.10;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(inner.left() + w * 0.16, header_y - h * 0.022),
                Pos2::new(inner.right() - w * 0.16, header_y + h * 0.022),
            ),
            2.0,
            feed,
        );
        for k in 0..7 {
            let hx = inner.left() + w * 0.20 + k as f32 * w * 0.10;
            painter.line_segment(
                [
                    Pos2::new(hx, header_y + h * 0.022),
                    Pos2::new(hx, header_y + h * 0.075),
                ],
                Stroke::new(1.6_f32, feed),
            );
        }
        self.tag(
            painter,
            Pos2::new(inner.center().x, header_y - h * 0.06),
            "feedwater header",
        );

        // Feedwater inlet nozzle, down through the top of the shell.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(inner.center().x + w * 0.20, rect.top() - h * 0.07),
                Pos2::new(inner.center().x + w * 0.235, header_y),
            ),
            2.0,
            feed,
        );

        // Steam nozzles along the top of the shell.
        for k in 0..4 {
            let nx = inner.left() + inner.width() * (0.18 + 0.21 * k as f32);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(nx - w * 0.018, rect.top() - h * 0.11),
                    Pos2::new(nx + w * 0.018, rect.top() + h * 0.06),
                ),
                2.0,
                steam,
            );
        }

        painter.rect_stroke(
            rect,
            radius(h * 0.5),
            Stroke::new(1.5_f32, OUTLINE),
            StrokeKind::Middle,
        );
    }
}

// ── 3. Helical coil (once-through) ──────────────────────────────────────────

impl SteamGeneratorVisual {
    /// Draws the once-through helical-coil generator.
    ///
    /// Follows the HTR-10 arrangement described in the module documentation:
    /// hot primary gas arrives through a **centre tube** to the top of the
    /// unit, turns, and flows **down** over the helical bundle before
    /// returning **up along the vessel wall** to the circulator; feedwater
    /// enters at the bottom and rises through the coil, leaving as
    /// superheated steam at the top. The two streams therefore run
    /// counter-current, which is why the coldest water meets the coldest gas.
    ///
    /// **No water level is drawn, because there is none** — a once-through
    /// generator has no free surface (see
    /// [`SteamGeneratorKind::has_water_level`]), and any level fraction passed
    /// in is ignored.
    ///
    /// The coil colour grades linearly from the feedwater temperature at the
    /// bottom to the steam temperature at the top. That is a **display
    /// interpolation, not a computed profile**: a real once-through tube runs
    /// subcooled, then nearly flat at saturation through the evaporator, then
    /// up into superheat, and computing that needs an enthalpy balance which
    /// belongs in `tampines`, not in this presentation crate. The zone labels
    /// name the three regions without pretending the gradient resolves them.
    fn draw_helical_coil(&self, painter: &Painter, rect: Rect, s: SteamGeneratorScalars) {
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let y = |f: f32| rect.top() + f * h;

        let primary_in = self.colour(s.primary_inlet_temp);
        let primary_out = self.colour(s.primary_outlet_temp);
        let feed = self.colour(s.feedwater_temp);
        let steam = self.colour(s.steam_temp);

        // ── Pressure vessel: a capsule, as the HTR-10 SG vessel is ─────────
        let dome = w * 0.5;
        let shell = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + dome * 0.62),
            Pos2::new(rect.right(), rect.bottom() - dome * 0.62),
        );
        painter.rect_filled(shell, 0.0, STEEL);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.right(), shell.top() + dome * 0.4),
            ),
            radius(dome * 0.6),
            STEEL,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), shell.bottom() - dome * 0.4),
                Pos2::new(rect.right(), rect.bottom()),
            ),
            radius(dome * 0.6),
            STEEL,
        );
        let interior = Rect::from_min_max(
            Pos2::new(cx - w * 0.45, y(0.06)),
            Pos2::new(cx + w * 0.45, y(0.94)),
        );
        painter.rect_filled(interior, radius(w * 0.06), VOID);

        // ── Shell-side gas, hottest at the top where it enters ─────────────
        let bundle_top = y(0.18);
        let bundle_bottom = y(0.80);
        let bands = 14;
        for b in 0..bands {
            let t = (b as f32 + 0.5) / bands as f32;
            let band = Rect::from_min_max(
                Pos2::new(
                    cx - w * 0.38,
                    bundle_top + (bundle_bottom - bundle_top) * b as f32 / bands as f32,
                ),
                Pos2::new(
                    cx + w * 0.38,
                    bundle_top + (bundle_bottom - bundle_top) * (b as f32 + 1.0) / bands as f32,
                ),
            );
            painter.rect_filled(
                band,
                0.0,
                translucent(
                    self.colour(lerp_temperature(
                        s.primary_inlet_temp,
                        s.primary_outlet_temp,
                        t,
                    )),
                    70,
                ),
            );
        }

        // ── Cold-gas return annulus, up the vessel wall to the circulator ──
        for side in [-1.0f32, 1.0] {
            let outer = cx + side * w * 0.44;
            let inner = cx + side * w * 0.385;
            let (left, right) = if side < 0.0 {
                (outer, inner)
            } else {
                (inner, outer)
            };
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(left, y(0.10)), Pos2::new(right, y(0.88))),
                2.0,
                translucent(primary_out, 215),
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.085)), "cold gas return");

        // ── Helical coil around the centre tube ────────────────────────────
        //
        // Drawn in two passes so the winding reads as a helix: the half of
        // each turn that passes BEHIND the centre column is drawn first and
        // darker, the column is drawn over it, then the front half is drawn at
        // full strength.
        let column_half = w * 0.06;
        let layers = [(w * 0.17, 0.0f32), (w * 0.30, PI * 0.5)];
        for behind in [true, false] {
            if !behind {
                // The centre tube carries the hot gas up to the top of the
                // unit, so it is drawn at the primary INLET temperature.
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(cx - column_half, y(0.12)),
                        Pos2::new(cx + column_half, y(0.90)),
                    ),
                    2.0,
                    primary_in,
                );
                painter.rect_stroke(
                    Rect::from_min_max(
                        Pos2::new(cx - column_half, y(0.12)),
                        Pos2::new(cx + column_half, y(0.90)),
                    ),
                    2.0,
                    Stroke::new(1.0_f32, INTERNALS),
                    StrokeKind::Middle,
                );
            }
            for (coil_radius, phase) in layers {
                let turns = 8.0;
                let samples = 160;
                let width = (w * 0.022).max(1.0);
                for k in 0..samples {
                    let t0 = k as f32 / samples as f32;
                    let t1 = (k + 1) as f32 / samples as f32;
                    let a0 = phase + turns * 2.0 * PI * t0;
                    let a1 = phase + turns * 2.0 * PI * t1;
                    let is_behind = (0.5 * (a0 + a1)).cos() < 0.0;
                    if is_behind != behind {
                        continue;
                    }
                    // t = 0 at the TOP of the drawing; the water rises, so the
                    // colour is taken from the height above the bundle bottom.
                    let p0 = Pos2::new(
                        cx + coil_radius * a0.sin(),
                        bundle_top + (bundle_bottom - bundle_top) * t0,
                    );
                    let p1 = Pos2::new(
                        cx + coil_radius * a1.sin(),
                        bundle_top + (bundle_bottom - bundle_top) * t1,
                    );
                    let rise = 1.0 - 0.5 * (t0 + t1);
                    let mut colour =
                        self.colour(lerp_temperature(s.feedwater_temp, s.steam_temp, rise));
                    if behind {
                        colour = translucent(colour, 130);
                    }
                    painter.line_segment([p0, p1], Stroke::new(width, colour));
                }
            }
        }
        self.tag(painter, Pos2::new(cx, y(0.145)), "helical coil");

        // Zone names. The gradient does not resolve them — they are named so a
        // reader knows where the phase change lives in a once-through unit.
        for (f, name) in [
            (0.26f32, "superheater"),
            (0.50, "evaporator"),
            (0.74, "economiser"),
        ] {
            self.tag(painter, Pos2::new(cx, y(f)), name);
        }
        self.tag(painter, Pos2::new(cx, y(0.855)), "once-through: no level");

        // ── Nozzles ────────────────────────────────────────────────────────
        // Superheated steam out at the top, feedwater in at the bottom.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx + w * 0.30, y(0.10)),
                Pos2::new(cx + w * 0.68, y(0.125)),
            ),
            2.0,
            steam,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - w * 0.68, y(0.885)),
                Pos2::new(cx - w * 0.30, y(0.91)),
            ),
            2.0,
            feed,
        );
        // Hot gas duct into the bottom of the centre tube.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx + w * 0.02, y(0.925)),
                Pos2::new(cx + w * 0.68, y(0.955)),
            ),
            2.0,
            primary_in,
        );
        // Cold gas to the circulator at the top.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - w * 0.68, y(0.045)),
                Pos2::new(cx - w * 0.10, y(0.075)),
            ),
            2.0,
            primary_out,
        );

        painter.rect_stroke(
            shell,
            0.0,
            Stroke::new(1.5_f32, OUTLINE),
            StrokeKind::Middle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
    use tampines::components::HeatExchanger;
    use tampines::hem::HemSteamCv;
    use uom::si::area::square_meter;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::f64::{Area, AvailableEnergy, HeatTransfer, Pressure, Volume};
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::pressure::megapascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::volume::cubic_meter;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn scalars() -> SteamGeneratorScalars {
        SteamGeneratorScalars {
            primary_inlet_temp: degc(320.0),
            primary_outlet_temp: degc(288.0),
            feedwater_temp: degc(226.0),
            steam_temp: degc(285.0),
            water_level_frac: 0.62,
        }
    }

    fn visual(kind: SteamGeneratorKind) -> SteamGeneratorVisual {
        SteamGeneratorVisual::from_scalars(
            kind,
            Pos2::new(100.0, 200.0),
            Vec2::new(200.0, 400.0),
            degc(100.0),
            degc(400.0),
            scalars(),
        )
    }

    /// Each architecture must keep its own real proportions at any box size.
    ///
    /// **Methodology.** The three envelopes are taken from published
    /// dimensions: a large Western PWR recirculating generator at 4.5 m over
    /// 20.6 m, the PGV-1000M horizontal vessel at 13.84 m over 4.0 m, and the
    /// HTR-10 steam-generator pressure vessel at 2.5 m over 11.3 m. Require
    /// each [`SteamGeneratorKind::native_aspect_ratio`] to equal its quotient,
    /// and require [`SteamGeneratorKind::fit_native_aspect`] to preserve that
    /// ratio — and stay centred — in a square box, an over-wide box and an
    /// over-tall box.
    ///
    /// **Result (2026-08-06):** ratios 0.21845 (vertical), 3.46000
    /// (horizontal) and 0.22124 (helical), each matching its quotient to
    /// better than 1e-6; all nine box/kind combinations preserved the ratio to
    /// better than 1e-4 and stayed centred to better than 1e-4 points, and the
    /// fitted rectangle never exceeded its box. Interpretation: the wide VVER
    /// generator stays wide in a tall card and the slender vertical and
    /// helical units stay slender in a square one, so the three read as
    /// different machines at any gallery size.
    #[test]
    fn each_kind_letterboxes_to_its_own_published_proportions() {
        assert!((VERTICAL_U_TUBE_ASPECT_RATIO - (4.5 / 20.6)).abs() < 1e-6);
        assert!((HORIZONTAL_U_TUBE_ASPECT_RATIO - (13.84 / 4.0)).abs() < 1e-6);
        assert!((HELICAL_COIL_ASPECT_RATIO - (2.5 / 11.3)).abs() < 1e-6);

        for kind in SteamGeneratorKind::ALL {
            for size in [
                Vec2::new(300.0, 300.0),
                Vec2::new(900.0, 200.0),
                Vec2::new(100.0, 900.0),
            ] {
                let box_rect = Rect::from_min_size(Pos2::new(17.0, 23.0), size);
                let fitted = kind.fit_native_aspect(box_rect);
                assert!(
                    (fitted.width() / fitted.height() - kind.native_aspect_ratio()).abs() < 1e-4,
                    "{:?} in box {size:?} did not preserve its ratio",
                    kind
                );
                assert!(
                    (fitted.center() - box_rect.center()).length() < 1e-4,
                    "{:?} in box {size:?} was not centred",
                    kind
                );
                assert!(
                    fitted.width() <= size.x + 1e-3 && fitted.height() <= size.y + 1e-3,
                    "{:?} in box {size:?} overflowed its box",
                    kind
                );
            }
        }
    }

    /// The horizontal generator must be the only wide one — the difference is
    /// physical, not stylistic: its free surface runs the length of the
    /// vessel, which is why it needs no separator stack.
    #[test]
    fn only_the_vver_generator_is_wider_than_it_is_tall() {
        assert!(SteamGeneratorKind::HorizontalUTube.native_aspect_ratio() > 1.0);
        assert!(SteamGeneratorKind::VerticalUTube.native_aspect_ratio() < 1.0);
        assert!(SteamGeneratorKind::HelicalCoil.native_aspect_ratio() < 1.0);
    }

    /// A degenerate box must not produce NaN geometry — zero-height
    /// allocations happen transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        for kind in SteamGeneratorKind::ALL {
            assert_eq!(kind.fit_native_aspect(flat), flat);
        }
    }

    /// The water level must always land inside `[0, 1]`, and the once-through
    /// generator must report that it has none.
    ///
    /// **Methodology.** A level comes from a controller that can transiently
    /// overshoot, and artwork must clamp rather than panic or draw off the
    /// vessel. Sweep [`drawn_water_level`] over -5.0..=5.0 in steps of 0.01
    /// for both recirculating kinds and require every result to lie in
    /// `[0, 1]`, to equal the input where the input is already in range, and
    /// to saturate at the ends. Require non-finite inputs to draw as empty
    /// (the most visible outcome, so a NaN cannot hide behind a plausible
    /// mid-drum level), and require `HelicalCoil` to return `None`.
    ///
    /// **Result (2026-08-06):** 2 002 sampled levels over the two
    /// recirculating kinds, all in `[0, 1]`; identity held exactly on
    /// `[0, 1]`; -5.0 gave 0.0 and 5.0 gave 1.0; NaN, +inf and -inf all gave
    /// 0.0; `HelicalCoil` gave `None` for every input. Interpretation: no
    /// caller value can put the level outside the vessel.
    #[test]
    fn the_water_level_clamps_to_the_unit_interval() {
        let mut sampled = 0usize;
        for kind in [
            SteamGeneratorKind::VerticalUTube,
            SteamGeneratorKind::HorizontalUTube,
        ] {
            assert!(kind.has_water_level());
            for step in -500..=500 {
                let input = step as f32 * 0.01;
                let level = drawn_water_level(kind, input).expect("kind has a level");
                assert!(
                    (0.0..=1.0).contains(&level),
                    "level {level} out of range for input {input}"
                );
                if (0.0..=1.0).contains(&input) {
                    assert_eq!(level, input, "in-range level must pass through");
                }
                sampled += 1;
            }
            assert_eq!(drawn_water_level(kind, -5.0), Some(0.0));
            assert_eq!(drawn_water_level(kind, 5.0), Some(1.0));
            assert_eq!(drawn_water_level(kind, f32::NAN), Some(0.0));
            assert_eq!(drawn_water_level(kind, f32::INFINITY), Some(0.0));
            assert_eq!(drawn_water_level(kind, f32::NEG_INFINITY), Some(0.0));
        }
        println!("{sampled} level samples checked");

        assert!(!SteamGeneratorKind::HelicalCoil.has_water_level());
        for input in [0.0f32, 0.5, 1.0, -2.0, f32::NAN] {
            assert_eq!(
                drawn_water_level(SteamGeneratorKind::HelicalCoil, input),
                None,
                "a once-through generator has no water level"
            );
        }
    }

    /// A level set on the widget must survive to the drawn scalars unchanged,
    /// so clamping happens at render time only — the same contract the reactor
    /// vessels use for control-rod insertion.
    #[test]
    fn the_level_is_stored_unclamped() {
        let v = visual(SteamGeneratorKind::VerticalUTube).with_water_level(1.7);
        assert_eq!(v.scalars().water_level_frac, 1.7);
        assert_eq!(drawn_water_level(v.kind(), 1.7), Some(1.0));
    }

    /// Every scatter in the artwork must be identical frame to frame.
    ///
    /// **Methodology.** The widget is consumed by value and rebuilt on every
    /// repaint, so droplets and tube sag drawn from a real random source would
    /// shimmer. Evaluate [`sg_hash`] repeatedly at 3 000 (index, salt) sites
    /// and require bitwise-equal results in `[0, 1)`; require the salted draws
    /// at one site to differ, or position and size would vary together;
    /// require adjacent indices to decorrelate, or the scatter would read as
    /// stripes; and require [`scatter_point`] to be bitwise stable and to land
    /// inside its rectangle.
    ///
    /// **Result (2026-08-06):** 3 000 hash sites re-evaluated three times each,
    /// all bitwise identical and in range; 38 of 40 adjacent index pairs
    /// differed by more than 0.05; the four salted draws at one site were
    /// pairwise distinct; 400 scatter points were stable across repeats and
    /// all lay inside their rectangle. Interpretation: the mist, the bubbles
    /// and the tube sag are stable across repaints while still looking
    /// random.
    #[test]
    fn the_scatter_is_deterministic_and_in_range() {
        let mut checked = 0usize;
        for index in -50..250 {
            for salt in 31..41u32 {
                let first = sg_hash(index, 0, salt);
                for _ in 0..3 {
                    assert_eq!(first, sg_hash(index, 0, salt), "hash is not deterministic");
                }
                assert!((0.0..1.0).contains(&first), "hash {first} out of range");
                checked += 1;
            }
        }
        println!("{checked} hash sites re-evaluated");

        let (a, b, c, d) = (
            sg_hash(7, 3, 31),
            sg_hash(7, 3, 32),
            sg_hash(7, 3, 33),
            sg_hash(7, 3, 34),
        );
        assert!((a - b).abs() > 1e-6 && (b - c).abs() > 1e-6 && (c - d).abs() > 1e-6);

        let mut differing = 0;
        for i in 0..40 {
            if (sg_hash(i, 0, 31) - sg_hash(i + 1, 0, 31)).abs() > 0.05 {
                differing += 1;
            }
        }
        println!("{differing}/40 adjacent index pairs decorrelated");
        assert!(
            differing > 30,
            "adjacent sites are too similar ({differing}/40 differ) — the scatter will look striped"
        );

        let area = Rect::from_min_size(Pos2::new(12.0, -30.0), Vec2::new(140.0, 55.0));
        for index in 0..200 {
            for salt in [31u32, 41] {
                let first = scatter_point(area, index, salt);
                assert_eq!(first, scatter_point(area, index, salt));
                assert!(
                    area.contains(first),
                    "scatter point {first:?} escaped its area"
                );
            }
        }
    }

    /// The U-tubes must nest inside the bundle, with the outermost tube
    /// reaching highest — that nesting is what gives a real bundle its domed
    /// top, and it is drawn rather than faked.
    ///
    /// **Methodology.** Each tube's U-bend is a semicircle whose radius equals
    /// its own leg offset, so the apex of tube `i` sits exactly that offset
    /// above the bend centreline. Require [`u_tube_leg_offset`] to be strictly
    /// increasing in the tube index, to stay strictly inside the bundle
    /// half-width, and to be proportional to the bundle half-width, for bundle
    /// counts 1..=20.
    ///
    /// **Result (2026-08-06):** strictly increasing for every count tested;
    /// the outermost tube of a seven-tube bundle sits at 0.9211 of the bundle
    /// half-width, leaving a margin of 7.9 % of the half-width to the wrapper;
    /// scaling was exactly proportional. Interpretation: no tube can be drawn
    /// outside the wrapper, and the bend apexes fan out into the domed bundle
    /// top.
    #[test]
    fn the_u_tubes_nest_inside_the_bundle() {
        for count in 1..=20usize {
            let half = 100.0f32;
            let mut previous = 0.0;
            for i in 0..count {
                let offset = u_tube_leg_offset(i, count, half);
                assert!(
                    offset > previous,
                    "tube {i} of {count} did not step outward ({offset} <= {previous})"
                );
                assert!(
                    offset < half,
                    "tube {i} of {count} escaped the bundle ({offset} >= {half})"
                );
                previous = offset;
            }
        }
        let outer = u_tube_leg_offset(6, 7, 100.0);
        println!("outermost tube of a 7-tube bundle at {outer:.4} of the half-width");
        assert!((outer - 92.105_263).abs() < 1e-3);
        // Proportional in the bundle width, so the nesting survives resizing.
        assert!(
            (u_tube_leg_offset(3, 7, 50.0) * 2.0 - u_tube_leg_offset(3, 7, 100.0)).abs() < 1e-4
        );
    }

    /// The colour gradient along a tube must interpolate in kelvin and hit its
    /// endpoints exactly, so a tube's ends read as the two primary
    /// temperatures the caller supplied and nothing else.
    #[test]
    fn the_tube_gradient_interpolates_between_its_endpoints() {
        let (from, to) = (degc(320.0), degc(288.0));
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

    /// The physics-backed path must read the **live** secondary-side
    /// temperature, not a value captured at construction.
    ///
    /// **Methodology.** Build a `tampines::components::SteamGenerator` around
    /// a secondary-side `HemSteamCv` flashed from 6.0 MPa and
    /// 2 780 kJ/kg, wrap it with `SteamGeneratorVisual::new` (the original
    /// constructor, whose signature is preserved), and require
    /// `scalars().steam_temp` to equal
    /// `secondary_side.get_temperature()` exactly. Then replace the control
    /// volume with a 3.0 MPa / 2 900 kJ/kg state and require the visual to
    /// follow it.
    ///
    /// **Result (2026-08-06):** both states matched their getter bit for bit —
    /// 275.586 degC at 6.0 MPa / 2 780 kJ/kg, which is just inside the
    /// two-phase dome (`h_g` is about 2 784 kJ/kg there) and so returns the
    /// saturation temperature, and 264.723 degC at 3.0 MPa / 2 900 kJ/kg,
    /// which is superheated (`h_g` about 2 804 kJ/kg). Interpretation: the
    /// physics path is live
    /// data, so a widget that silently ignored its control volume would fail
    /// here.
    #[test]
    fn the_physics_path_reads_the_live_secondary_temperature() {
        let volume = Volume::new::<cubic_meter>(1.0);
        let steam = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(6.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_780_000.0),
            volume,
        );
        let exchanger = HeatExchanger::new(
            FlowArrangement::CounterCurrent,
            Area::new::<square_meter>(800.0),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(1500.0),
        );
        let generator = SteamGenerator::new(exchanger, steam);
        let expected = generator.secondary_side.get_temperature();

        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        assert_eq!(visual.scalars().steam_temp, expected);
        println!(
            "6.0 MPa / 2780 kJ/kg -> {:.3} degC",
            expected.get::<degree_celsius>()
        );

        // A different state must give a different drawn temperature.
        let hotter = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(3.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_900_000.0),
            volume,
        );
        let generator = SteamGenerator::new(exchanger, hotter);
        let expected_hot = generator.secondary_side.get_temperature();
        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        assert_eq!(visual.scalars().steam_temp, expected_hot);
        println!(
            "3.0 MPa / 2900 kJ/kg -> {:.3} degC",
            expected_hot.get::<degree_celsius>()
        );
        assert!(expected_hot != expected);
    }

    /// The physics constructor must not fabricate a primary side it cannot
    /// see: it starts isothermal at the secondary temperature, and the
    /// builders replace those values.
    #[test]
    fn the_physics_constructor_starts_isothermal_until_told_otherwise() {
        let steam = HemSteamCv::new_from_ph(
            Pressure::new::<megapascal>(6.0),
            AvailableEnergy::new::<joule_per_kilogram>(2_780_000.0),
            Volume::new::<cubic_meter>(1.0),
        );
        let exchanger = HeatExchanger::new(
            FlowArrangement::CounterCurrent,
            Area::new::<square_meter>(800.0),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(1500.0),
        );
        let generator = SteamGenerator::new(exchanger, steam);
        let secondary = generator.secondary_side.get_temperature();

        let visual = SteamGeneratorVisual::new(
            generator,
            Pos2::ZERO,
            Vec2::new(120.0, 240.0),
            degc(100.0),
            degc(400.0),
        );
        let drawn = visual.scalars();
        assert_eq!(drawn.primary_inlet_temp, secondary);
        assert_eq!(drawn.primary_outlet_temp, secondary);
        assert_eq!(drawn.feedwater_temp, secondary);
        assert_eq!(visual.kind(), SteamGeneratorKind::VerticalUTube);

        let visual = visual
            .with_primary_temperatures(degc(320.0), degc(288.0))
            .with_feedwater_temperature(degc(226.0))
            .with_kind(SteamGeneratorKind::HelicalCoil);
        let drawn = visual.scalars();
        assert_eq!(drawn.primary_inlet_temp, degc(320.0));
        assert_eq!(drawn.primary_outlet_temp, degc(288.0));
        assert_eq!(drawn.feedwater_temp, degc(226.0));
        assert_eq!(
            drawn.steam_temp, secondary,
            "the live steam path must survive"
        );
        assert_eq!(visual.kind(), SteamGeneratorKind::HelicalCoil);
    }

    /// The scalar path must pass the caller's state through untouched — this
    /// is real model state, not a placeholder, so nothing may be substituted.
    #[test]
    fn the_scalar_path_passes_state_through_unchanged() {
        for kind in SteamGeneratorKind::ALL {
            let v = visual(*kind);
            assert_eq!(v.scalars(), scalars());
            assert_eq!(v.size(), Vec2::new(200.0, 400.0));
            assert_eq!(v.kind(), *kind);
        }
    }

    /// Every kind must name itself, its reactor family and its circulation
    /// mode, so a gallery caption can be built without a lookup table
    /// elsewhere going stale.
    #[test]
    fn every_kind_describes_itself() {
        assert_eq!(SteamGeneratorKind::ALL.len(), 3);
        for kind in SteamGeneratorKind::ALL {
            assert!(!kind.label().is_empty());
            assert!(!kind.description().is_empty());
            assert!(!kind.circulation().is_empty());
        }
        assert!(SteamGeneratorKind::HelicalCoil
            .circulation()
            .contains("once-through"));
        assert!(SteamGeneratorKind::VerticalUTube
            .circulation()
            .contains("recirculating"));
    }

    /// Corner radii are `u8` in egui, so the helper must saturate rather than
    /// wrap — a wrapped radius would round a huge vessel end to nothing.
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
