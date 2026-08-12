//! Schematic cooling-tower art, one architecture per draught type.
//!
//! A cooling tower is a **psychrometric** machine, not a heat exchanger with a
//! wall in it: warm circulating water is broken up over a fill pack and put in
//! direct contact with air, and most of the cooling comes from evaporating a
//! little of that water into the air stream. Two consequences set everything
//! this widget draws.
//!
//! - **The wet-bulb temperature governs the machine.** Evaporation can cool the
//!   water towards the air's wet-bulb temperature but never below it, so the
//!   number that says how good a tower is doing is the **approach** —
//!   `T_water,out - T_wb` — not the dry-bulb temperature. See
//!   [`approach_to_wet_bulb`]. The other headline number is the **range**,
//!   `T_water,in - T_water,out`, which is set by the heat load rather than by
//!   the tower ([`cooling_range`]).
//! - **The plume is condensed water, not steam.** Air leaving the fill is at or
//!   near saturation; when it mixes with cooler ambient air some of the water
//!   it carries condenses into visible droplets. So how visible the plume is
//!   depends on how far the exit air sits into saturation — which is why
//!   [`plume_opacity`] is driven by the exit air's *relative humidity* and by
//!   nothing else.
//!
//! # Where the psychrometrics come from
//!
//! Both air states are [`tampines::humid_air::HumidAirState`], which is a
//! `uom`-typed wrapper over
//! `outram_park_fork_coolprop::humid_air::ha_props` — this workspace's
//! `HAPropsSI` port, humid air as a real-gas mixture per ASHRAE RP-1485. The
//! widget therefore reads **real psychrometric properties** (dry-bulb,
//! pressure, humidity ratio, relative humidity, enthalpy, specific volume) that
//! the caller resolved through that backend, rather than anything invented
//! here.
//!
//! **The wet-bulb temperature is the one exception, and it is supplied by the
//! caller.** The CoolProp backend can produce it
//! (`HumidAirParam::TWetBulb`), but [`tampines::humid_air::HumidAirState`] has
//! no field for it and `tampines` does not re-export the raw `ha_props` entry
//! point, so there is nothing on the state object to read. Solving for it here
//! would be new physics inside a presentation crate, which this crate's
//! `CLAUDE.md` forbids — the fix belongs in `tampines`, not in this file. Until
//! then [`CoolingTowerScalars::inlet_wet_bulb`] is a caller-supplied scalar,
//! exactly like every other quantity on the
//! [`crate::components::PipeVisual::from_scalars`] path: real state from the
//! caller's own model, not a placeholder.
//!
//! # What the physics component can and cannot supply
//!
//! [`tampines::components::CoolingTower`] holds a **real** ambient air inlet
//! state, a real circulating-water inlet temperature and flow rate, and a
//! **target** approach — a set-point. Its `evaluate` returns
//! `TampinesError::NotYetImplemented`, so there is no water outlet temperature
//! and **no exit air state at all**.
//!
//! [`CoolingTowerVisual::new`] therefore draws the air inlet and the hot water
//! it really knows about, and leaves everything downstream of the fill
//! deliberately blank: the basin is neutral grey, no approach is reported (only
//! the target, labelled as a target), and **no plume is drawn**. An invented
//! ambient condition or an assumed saturated exit would make a plume appear out
//! of nothing, which is exactly the failure this crate refuses. For a fully
//! painted tower, pass the state you actually have to
//! [`CoolingTowerVisual::from_scalars`].
//!
//! # Dispatch
//!
//! [`CoolingTowerKind`] and [`CoolingTowerVisualState`] are enums, not trait
//! objects, per the workspace's mandatory "no trait objects" Rust design rule.
//!
//! # Simulation time is application-owned
//!
//! An induced-draught tower has a fan, and it is drawn at
//! `theta = omega * t` from a caller-supplied shaft speed and the
//! **application's** simulation clock — the same contract as
//! [`crate::components::PumpVisual`] and
//! [`crate::components::TurbineVisual`], and for the same reason: widgets are
//! rebuilt every repaint, so a clock owned by the widget would reset to zero
//! each frame. `CoolingTower` carries no fan speed, so the default is zero and
//! the fan is drawn **stationary but complete** rather than at a fabricated
//! speed.
//!
//! # What this is not
//!
//! **Offline demonstration artwork, not a validated model and not a design
//! drawing.** The hyperboloid shell really is drawn from the hyperbola that
//! defines a hyperboloid of one sheet ([`hyperboloid_half_width`]), but its
//! proportions — and every other proportion in this module — are chosen by eye
//! for legibility and are dimensioned from no design. Nothing here may be cited
//! or re-used as cooling-tower design data. Per `RESPONSIBLE_USE.md` this is
//! for education, research and V&V only.

use crate::components::temperature_colour;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape};
use egui::{Stroke, StrokeKind, Ui, Vec2, Widget};
use std::f32::consts::TAU;
use tampines::components::CoolingTower;
use tampines::humid_air::HumidAirState;
use uom::si::f64::{
    Angle, AngularVelocity, Ratio, TemperatureInterval, ThermodynamicTemperature, Time, VolumeRate,
};
use uom::si::ratio::{percent, ratio};
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume_rate::cubic_meter_per_second;
use uom::ConstZero;

// ── Envelope proportions ────────────────────────────────────────────────────

/// Width-to-height ratio of the natural-draught (hyperbolic) tower,
/// dimensionless.
///
/// **Chosen by eye, not taken from a design.** A natural-draught shell is
/// taller than it is wide because the chimney height *is* the draught: the
/// column of warm, moist — and therefore less dense — air inside the shell is
/// what pulls fresh air through the fill, so there is no fan.
pub const NATURAL_DRAUGHT_ASPECT_RATIO: f32 = 0.72;

/// Width-to-height ratio of the mechanical induced-draught tower,
/// dimensionless.
///
/// **Chosen by eye, not taken from a design.** A fan-driven cell needs no
/// chimney, so it is a low broad box with the fan on the roof.
pub const INDUCED_DRAUGHT_ASPECT_RATIO: f32 = 1.45;

/// Height fraction of the hyperboloid's throat, measured **from the top** of
/// the drawing (`0.0` the top rim, `1.0` the base). Dimensionless.
///
/// The waist sits well up the shell, with a short flare above it to the cornice
/// and a long flare below it to the air inlet.
pub const HYPERBOLOID_THROAT_FRACTION: f32 = 0.16;

/// Flare parameter of the hyperboloid meridian, dimensionless — the `b` in
/// `x = a * sqrt(1 + (y / b)^2)`, in units of the drawn height.
///
/// Smaller values flare faster. Chosen so the base is about 1.45 times the
/// throat width, which reads as a cooling tower rather than as a chimney.
pub const HYPERBOLOID_FLARE: f32 = 0.80;

/// Exit-air relative humidity below which no plume is drawn, dimensionless.
///
/// See [`plume_opacity`] — this is a **display threshold**, not a
/// plume-formation criterion.
pub const PLUME_VISIBLE_RH_MIN: f32 = 0.90;

/// Which cooling-tower architecture to draw.
///
/// The two differ in **how the draught is produced**, which is why one is a
/// 150-metre concrete chimney and the other a box with a fan on it. What
/// happens inside — hot water sprayed over a fill pack, air passing through it,
/// cooled water collected in a basin — is the same in both, and is drawn by the
/// same code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoolingTowerKind {
    /// Natural-draught hyperbolic tower: no fan at all.
    ///
    /// The buoyancy of the warm, moist air inside the shell drives the flow, so
    /// the shell has to be tall. Its hyperboloid form is structural — a
    /// hyperboloid of one sheet is a doubly-ruled surface, so a thin shell can
    /// be built from straight reinforcement and still resist wind. Drawn from
    /// the meridian hyperbola in [`hyperboloid_half_width`].
    NaturalDraught,
    /// Mechanical induced-draught cell: a fan on the roof pulls air up through
    /// the fill.
    ///
    /// "Induced" means the fan is downstream of the fill and works in the warm
    /// wet air leaving it, which is why the fan sits above the drift
    /// eliminators in the drawing.
    InducedDraught,
}

impl CoolingTowerKind {
    /// Every architecture, in the order a gallery should show them.
    pub const ALL: &'static [Self] = &[Self::NaturalDraught, Self::InducedDraught];

    /// Short display name, for a picker or a card caption.
    pub fn label(self) -> &'static str {
        match self {
            Self::NaturalDraught => "Natural-draught hyperbolic",
            Self::InducedDraught => "Mechanical induced-draught",
        }
    }

    /// What drives the air through the fill — the one fact that explains every
    /// geometric difference between the variants.
    pub fn draught(self) -> &'static str {
        match self {
            Self::NaturalDraught => "buoyancy of the warm moist air in the shell",
            Self::InducedDraught => "a fan on the roof, downstream of the fill",
        }
    }

    /// Where this architecture is normally used, in words.
    pub fn description(self) -> &'static str {
        match self {
            Self::NaturalDraught => "large base-load stations, no fan power",
            Self::InducedDraught => "modular cells, smaller footprint, fan power",
        }
    }

    /// Whether this architecture has a fan to draw.
    ///
    /// `false` for [`Self::NaturalDraught`], which is the whole point of it: a
    /// fan speed passed to a natural-draught tower is ignored, and
    /// [`CoolingTowerVisual::fan_angle`] returns `None`.
    pub fn has_fan(self) -> bool {
        match self {
            Self::NaturalDraught => false,
            Self::InducedDraught => true,
        }
    }

    /// Width-to-height ratio the artwork is drawn at, dimensionless.
    pub fn native_aspect_ratio(self) -> f32 {
        match self {
            Self::NaturalDraught => NATURAL_DRAUGHT_ASPECT_RATIO,
            Self::InducedDraught => INDUCED_DRAUGHT_ASPECT_RATIO,
        }
    }

    /// The largest sub-rectangle of `available` carrying this kind's
    /// [`Self::native_aspect_ratio`], centred within it.
    ///
    /// Same letterbox contract as the steam generators, condensers and reactor
    /// vessels: the artwork keeps its proportions at any box size rather than
    /// stretching to fill it, so a hyperbolic shell stays tall in a wide card
    /// and a fan cell stays squat in a tall one. A degenerate box (zero or
    /// negative extent, as egui layout can transiently produce) is returned
    /// unchanged rather than producing NaN geometry.
    pub fn fit_native_aspect(self, available: Rect) -> Rect {
        let (w, h) = (available.width(), available.height());
        if w <= 0.0 || h <= 0.0 {
            return available;
        }
        let aspect = self.native_aspect_ratio();
        let (fw, fh) = if w / h > aspect {
            (h * aspect, h)
        } else {
            (w, w / aspect)
        };
        Rect::from_center_size(available.center(), Vec2::new(fw, fh))
    }
}

// ── Psychrometric quantities the drawing is read against ────────────────────

/// The **approach**: how far the cold water leaving the tower sits above the
/// entering air's wet-bulb temperature, `T_water,out - T_wb`.
///
/// This is the number that governs a cooling tower. Evaporative cooling drives
/// the water towards the wet-bulb temperature and cannot pass it, so a smaller
/// approach means a better (or more generously sized, or more lightly loaded)
/// tower. Both arguments are absolute thermodynamic temperatures (`uom`-typed,
/// kelvin internally); the result is a [`TemperatureInterval`], because a
/// difference of two temperatures is an interval and not a temperature.
///
/// **A non-positive approach is not clamped away.** It means the caller's model
/// has the water leaving at or below the wet-bulb temperature, which a real
/// tower cannot do, and the widget displays it as it is so the reader can see
/// the model is out of range. Hiding it would be the more dangerous choice.
pub fn approach_to_wet_bulb(
    water_outlet: ThermodynamicTemperature,
    wet_bulb: ThermodynamicTemperature,
) -> TemperatureInterval {
    TemperatureInterval::new::<kelvin_interval>(
        water_outlet.get::<kelvin>() - wet_bulb.get::<kelvin>(),
    )
}

/// The **range**: how much the circulating water is cooled across the tower,
/// `T_water,in - T_water,out`.
///
/// Unlike the approach, the range is set by the heat load and the water flow
/// rate rather than by the tower itself — a tower does not "choose" its range.
/// Both arguments are absolute thermodynamic temperatures; the result is a
/// [`TemperatureInterval`].
pub fn cooling_range(
    water_inlet: ThermodynamicTemperature,
    water_outlet: ThermodynamicTemperature,
) -> TemperatureInterval {
    TemperatureInterval::new::<kelvin_interval>(
        water_inlet.get::<kelvin>() - water_outlet.get::<kelvin>(),
    )
}

/// How strongly the exit plume is drawn, dimensionless in `[0, 1]`, from the
/// exit air's relative humidity.
///
/// `0.0` draws no plume at all, `1.0` the densest one. The ramp runs from
/// [`PLUME_VISIBLE_RH_MIN`] to saturation (`R = 1`), so air leaving the fill
/// well short of saturation carries its water invisibly and air leaving at
/// saturation gives a full plume.
///
/// **This is a display mapping of a real supplied property, not a plume model.**
/// Whether a plume is actually visible depends on the mixing line between the
/// exit air and the ambient air crossing the saturation curve, which is a
/// psychrometric mixing calculation and belongs in `tampines`, not in this
/// presentation crate. What is honest to say — and all that is claimed here —
/// is that a plume needs the exit air to be at or very near saturation, and
/// this ramp shows how near it is.
///
/// A relative humidity above 1 saturates at full opacity rather than growing
/// further: `HumidAirState` describes single-phase moist air, so exit air
/// already carrying condensed droplets is outside what the caller's own state
/// object can represent. A **non-finite** relative humidity draws no plume,
/// which is the visible outcome — a NaN in the caller's model must not appear
/// as a confident cloud.
pub fn plume_opacity(relative_humidity: Ratio) -> f32 {
    let r = relative_humidity.get::<ratio>() as f32;
    if !r.is_finite() {
        return 0.0;
    }
    ((r - PLUME_VISIBLE_RH_MIN) / (1.0 - PLUME_VISIBLE_RH_MIN)).clamp(0.0, 1.0)
}

/// Half-width of the hyperboloid shell at height fraction `f`, in screen
/// points.
///
/// `f` is measured **from the top** of the drawing: `0.0` at the top rim,
/// `1.0` at the base. The meridian of a hyperboloid of one sheet is the
/// hyperbola
///
/// ```text
/// x(y) = throat_half * sqrt(1 + ((f - throat_fraction) / flare)^2)
/// ```
///
/// so the shell is genuinely drawn from the curve that defines the surface
/// rather than sketched freehand: the minimum is exactly at the throat, and the
/// flare is symmetric in the *distance* from the throat, which is what gives a
/// cooling tower its waist.
///
/// `throat_half` is the half-width at the waist in screen points and `flare`
/// (see [`HYPERBOLOID_FLARE`]) is in units of the drawn height — smaller values
/// flare faster. A non-positive `flare` would divide by zero, so it is treated
/// as a straight cylinder of constant `throat_half`.
pub fn hyperboloid_half_width(f: f32, throat_fraction: f32, throat_half: f32, flare: f32) -> f32 {
    if !(flare > 0.0) {
        return throat_half;
    }
    let u = (f - throat_fraction) / flare;
    throat_half * (1.0 + u * u).sqrt()
}

// ── Deterministic scatter ───────────────────────────────────────────────────

/// Deterministic pseudo-random value in `[0, 1)` from two indices and a salt.
///
/// **Determinism is the point.** Widgets here are consumed by value and rebuilt
/// on every repaint, so the rain zone and the plume drawn from a real random
/// source would boil and shimmer between frames. Hashing the indices instead
/// gives a scatter that looks random but is identical frame to frame.
///
/// Same integer-hash construction as `steam_generator::sg_hash`,
/// `condenser::condenser_hash` and `pump::pump_hash`, duplicated rather than
/// shared because those are private to their own modules and this one must not
/// reach into them. The salts used here are this module's own.
fn tower_hash(a: i32, b: i32, salt: u32) -> f32 {
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
/// Used for the falling droplets in the rain zone and for the puffs that make
/// up the plume. `salt` picks an independent scatter, so two features drawn in
/// overlapping regions do not land on top of each other. The point is always
/// inside `area` (inclusive of its edges) provided `area` is non-degenerate,
/// and is a pure function of `(index, salt)` — see [`tower_hash`].
fn scatter_point(area: Rect, index: i32, salt: u32) -> Pos2 {
    Pos2::new(
        area.left() + tower_hash(index, 0, salt) * area.width(),
        area.top() + tower_hash(index, 1, salt.wrapping_add(1)) * area.height(),
    )
}

/// Linear interpolation between two temperatures, in kelvin.
///
/// `t` is a dimensionless position along whatever path is being coloured,
/// clamped to `[0, 1]`. **This is a display interpolation, not physics**: the
/// real water temperature profile down a fill pack comes from a Merkel/NTU
/// balance, which `tampines::cooling_tower` is the intended home for and which
/// does not exist yet.
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

/// Reinforced concrete: the hyperbolic shell and the basin walls.
const CONCRETE: Color32 = Color32::from_rgb(122, 122, 118);
/// Structural steel and casing panels, matching `pump::CASING` and
/// `steam_generator::STEEL` so a tower reads as the same material family as the
/// rest of a schematic.
const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
/// Silhouette outline, drawn last so the shape reads on top.
const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
/// Internals that carry no interesting temperature: fill pack, drift
/// eliminators, louvres, supports, fan hub.
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
/// Unfilled interior, behind the coloured regions.
const VOID: Color32 = Color32::from_rgb(28, 30, 34);
/// Label text.
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);
/// The plume: condensed water droplets, so white rather than any point on a
/// temperature scale.
const PLUME: Color32 = Color32::from_rgb(232, 236, 240);
/// Fluid colour used when no state is supplied. Neutral grey is the honest
/// drawing of "not known" and is deliberately not a point on the temperature
/// scale. Same convention as `pump::UNKNOWN_FLUID`.
const UNKNOWN_FLUID: Color32 = Color32::GRAY;

// ── State ───────────────────────────────────────────────────────────────────

/// Scalar state of a cooling tower, as the caller's own model holds it.
///
/// Every field is **real state the caller already has** — see the module
/// documentation and [`crate::components::PipeVisual::from_scalars`] for why
/// this narrower interface exists. Nothing here is invented by the widget.
///
/// The two air states are full [`tampines::humid_air::HumidAirState`] values,
/// so the caller resolves them through the CoolProp-backed psychrometrics
/// (`tampines::humid_air::state_from_t_p_r` and friends) and this widget only
/// reads properties off them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolingTowerScalars {
    /// Ambient air drawn in at the base.
    ///
    /// Its dry-bulb temperature colours the inlet-air arrows and its relative
    /// humidity is reported next to them.
    pub air_inlet: HumidAirState,
    /// Air leaving the tower, past the drift eliminators.
    ///
    /// Its dry-bulb temperature colours the exit-air arrows and its **relative
    /// humidity drives the plume** through [`plume_opacity`].
    pub air_outlet: HumidAirState,
    /// Wet-bulb temperature of the entering air.
    ///
    /// Supplied by the caller because [`HumidAirState`] carries no wet-bulb
    /// field — see the module documentation. Sets the approach through
    /// [`approach_to_wet_bulb`], and is drawn as a marker on the water-side
    /// scale that the cold water can approach but not cross.
    pub inlet_wet_bulb: ThermodynamicTemperature,
    /// Warm water returning from the plant to the distribution deck.
    pub water_inlet_temp: ThermodynamicTemperature,
    /// Cooled water leaving the basin.
    pub water_outlet_temp: ThermodynamicTemperature,
    /// Circulating-water volumetric flow rate.
    ///
    /// A **zero** flow draws no spray and no rain: a tower with nothing
    /// circulating through it is not cooling anything, and drawing water
    /// falling through it would be a lie about the plant's state.
    pub water_flow_rate: VolumeRate,
}

/// Where a [`CoolingTowerVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoolingTowerVisualState {
    /// Backed by a [`tampines::components::CoolingTower`].
    ///
    /// Its air inlet state, water inlet temperature and water flow rate are
    /// real and are drawn. Its `evaluate` is not implemented, so there is no
    /// water outlet temperature and no exit air state — the basin stays
    /// neutral, no approach is reported, and no plume is drawn. See the module
    /// documentation.
    Physics(CoolingTower),
    /// Backed by caller-supplied scalars from the caller's own plant model.
    Scalars(CoolingTowerScalars),
}

/// The quantities the artwork is actually painted from, with `None` wherever no
/// honest source exists.
///
/// Resolved once per repaint so every "we do not know this" decision is taken
/// in one place rather than scattered through the drawing code.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DrawnTower {
    air_inlet: Option<HumidAirState>,
    air_outlet: Option<HumidAirState>,
    inlet_wet_bulb: Option<ThermodynamicTemperature>,
    water_inlet_temp: Option<ThermodynamicTemperature>,
    water_outlet_temp: Option<ThermodynamicTemperature>,
    water_flow_rate: Option<VolumeRate>,
    /// The component's **target** approach, a set-point rather than an achieved
    /// value. Reported in words as a target and never used to colour anything.
    target_approach: Option<TemperatureInterval>,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
}

impl DrawnTower {
    /// Colour for a temperature, or [`UNKNOWN_FLUID`] if it is not known.
    fn colour(&self, t: Option<ThermodynamicTemperature>) -> Color32 {
        match t {
            Some(t) => temperature_colour(t, self.min_temp, self.max_temp),
            None => UNKNOWN_FLUID,
        }
    }

    /// Water colour a fraction `f` of the way down from the distribution deck
    /// (`0.0`) to the basin (`1.0`).
    ///
    /// A display interpolation between the two supplied temperatures (see
    /// [`lerp_temperature`]), so the rain visibly cools as it falls; neutral
    /// grey when either end is unknown, which is the whole fall on the
    /// physics-backed path.
    fn falling_water_colour(&self, f: f32) -> Color32 {
        match (self.water_inlet_temp, self.water_outlet_temp) {
            (Some(hot), Some(cold)) => self.colour(Some(lerp_temperature(hot, cold, f))),
            _ => UNKNOWN_FLUID,
        }
    }

    /// Whether water is circulating at all. `None` (no flow known) counts as
    /// not circulating, so nothing is drawn falling.
    fn is_circulating(&self) -> bool {
        matches!(self.water_flow_rate, Some(q) if q != VolumeRate::ZERO)
    }

    /// Opacity of the plume, `0.0` when there is no exit air state to judge it
    /// from — which is the physics-backed path, where drawing a plume would
    /// mean inventing the exit air.
    fn plume(&self) -> f32 {
        match self.air_outlet {
            Some(air) => plume_opacity(air.relative_humidity),
            None => 0.0,
        }
    }
}

impl CoolingTowerVisualState {
    /// The quantities the artwork is drawn from, and the display range they are
    /// graded against.
    fn resolve(
        &self,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> DrawnTower {
        match self {
            Self::Physics(tower) => DrawnTower {
                // Real, CoolProp-resolved state on the component.
                air_inlet: Some(tower.air_inlet),
                water_inlet_temp: Some(tower.water_inlet_temperature),
                water_flow_rate: Some(tower.water_flow_rate),
                target_approach: Some(tower.target_approach),
                // `CoolingTower::evaluate` is not implemented: there is no
                // outlet water temperature and no exit air state to draw.
                air_outlet: None,
                inlet_wet_bulb: None,
                water_outlet_temp: None,
                min_temp,
                max_temp,
            },
            Self::Scalars(s) => DrawnTower {
                air_inlet: Some(s.air_inlet),
                air_outlet: Some(s.air_outlet),
                inlet_wet_bulb: Some(s.inlet_wet_bulb),
                water_inlet_temp: Some(s.water_inlet_temp),
                water_outlet_temp: Some(s.water_outlet_temp),
                water_flow_rate: Some(s.water_flow_rate),
                target_approach: None,
                min_temp,
                max_temp,
            },
        }
    }
}

// ── The widget ──────────────────────────────────────────────────────────────

/// Visual representation of a cooling tower, in one of two draught
/// architectures.
///
/// Built either from a [`tampines::components::CoolingTower`] ([`Self::new`],
/// whose signature is preserved) or from the caller's own psychrometric plant
/// state ([`Self::from_scalars`]). See the module documentation for what each
/// path is allowed to paint — in particular, why the physics path draws no
/// plume.
///
/// All temperatures are absolute thermodynamic temperatures (`uom`-typed).
/// `min_temp`/`max_temp` bound the diverging colour scale; because the map is
/// diverging (blue at min, neutral white at the *midpoint*, red at max), set
/// them about a reference that matters rather than clamping to the extremes
/// seen.
pub struct CoolingTowerVisual {
    kind: CoolingTowerKind,
    state: CoolingTowerVisualState,
    screen_position: Pos2,
    screen_vector: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    fan_speed: AngularVelocity,
    simulation_time: Time,
    show_labels: bool,
}

impl CoolingTowerVisual {
    /// Wrap a [`CoolingTower`] with the given screen geometry and
    /// colour-mapping temperature range.
    ///
    /// **Signature preserved** from the original placeholder widget, so every
    /// existing call site keeps working unchanged.
    ///
    /// This is the physics-backed path. The component's ambient air inlet, hot
    /// water inlet temperature and water flow rate are real and are drawn; its
    /// `evaluate` is not implemented, so the cold water leaving the basin and
    /// the air leaving the top are **not known** and are left neutral, with no
    /// plume. Nothing is fabricated to fill the gap.
    ///
    /// `screen_position` is the **centre** of the box the artwork is
    /// letterboxed into, and `screen_vector` its size in screen points.
    ///
    /// Defaults to [`CoolingTowerKind::NaturalDraught`]; change it with
    /// [`Self::with_kind`].
    pub fn new(
        physics: CoolingTower,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            kind: CoolingTowerKind::NaturalDraught,
            state: CoolingTowerVisualState::Physics(physics),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            fan_speed: AngularVelocity::ZERO,
            simulation_time: Time::ZERO,
            show_labels: true,
        }
    }

    /// Build a cooling-tower visual from the caller's own psychrometric plant
    /// state.
    ///
    /// The state-driven path described in the module documentation: the caller
    /// passes the two CoolProp-resolved humid-air states, the entering
    /// wet-bulb temperature, the two water temperatures and the circulating
    /// flow from its own model, and the artwork grades itself against
    /// `min_temp`/`max_temp`.
    pub fn from_scalars(
        kind: CoolingTowerKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        scalars: CoolingTowerScalars,
    ) -> Self {
        Self {
            kind,
            state: CoolingTowerVisualState::Scalars(scalars),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            fan_speed: AngularVelocity::ZERO,
            simulation_time: Time::ZERO,
            show_labels: true,
        }
    }

    /// Draw a different draught architecture with the same state.
    pub fn with_kind(mut self, kind: CoolingTowerKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the fan shaft speed. Builder-style.
    ///
    /// Only [`CoolingTowerKind::InducedDraught`] has a fan; a speed set on a
    /// natural-draught tower is stored but never drawn, because that machine
    /// has no fan to turn. `CoolingTower` carries no fan speed of its own, so
    /// this is the only way a fan turns — and the default of zero draws a
    /// complete but stationary fan rather than a fabricated speed.
    pub fn with_fan_speed(mut self, fan_speed: AngularVelocity) -> Self {
        self.fan_speed = fan_speed;
        self
    }

    /// Supply the **application's** simulation time, which sets the fan phase
    /// `theta = omega * t`. Builder-style. See the module documentation for why
    /// the clock is application-owned.
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.simulation_time = simulation_time;
        self
    }

    /// Turn the internal component labels and readouts off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Which draught architecture this visual draws.
    pub fn kind(&self) -> CoolingTowerKind {
        self.kind
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.screen_vector
    }

    /// Where this visual gets its state.
    pub fn state(&self) -> &CoolingTowerVisualState {
        &self.state
    }

    /// The scalar state the artwork is drawn from, or `None` on the
    /// physics-backed path — which has none, and says so rather than
    /// synthesising one.
    pub fn scalars(&self) -> Option<CoolingTowerScalars> {
        match self.state {
            CoolingTowerVisualState::Physics(_) => None,
            CoolingTowerVisualState::Scalars(s) => Some(s),
        }
    }

    /// The wrapped component, or `None` on the scalar path.
    pub fn physics(&self) -> Option<CoolingTower> {
        match self.state {
            CoolingTowerVisualState::Physics(c) => Some(c),
            CoolingTowerVisualState::Scalars(_) => None,
        }
    }

    /// Current fan phase angle, `theta = omega * t`, or `None` for a kind with
    /// no fan.
    ///
    /// The identity is exact in `uom`'s type algebra — an angular velocity
    /// multiplied by a time *is* an angle — so nothing here is a tuned
    /// animation rate. Zero speed gives exactly zero phase at any time, and a
    /// negative speed runs the fan backwards rather than stopping it.
    pub fn fan_angle(&self) -> Option<Angle> {
        if !self.kind.has_fan() {
            return None;
        }
        Some((self.fan_speed * self.simulation_time).into())
    }

    /// The **achieved** approach to the entering wet-bulb temperature, or
    /// `None` when it cannot be known.
    ///
    /// `None` on the physics-backed path: `CoolingTower::evaluate` is not
    /// implemented, so there is no water outlet temperature, and the component
    /// carries no wet-bulb temperature either. The component's *target*
    /// approach is a set-point and is reported separately by
    /// [`Self::target_approach`] — the two must never be confused, which is why
    /// they are different methods.
    pub fn approach(&self) -> Option<TemperatureInterval> {
        let s = self.scalars()?;
        Some(approach_to_wet_bulb(s.water_outlet_temp, s.inlet_wet_bulb))
    }

    /// The wrapped component's **target** approach — a set-point, not an
    /// achieved value — or `None` on the scalar path.
    ///
    /// Displayed only with the word "target" beside it and never used to colour
    /// anything: painting the basin by a target would be drawing a result the
    /// plant has not produced.
    pub fn target_approach(&self) -> Option<TemperatureInterval> {
        match self.state {
            CoolingTowerVisualState::Physics(c) => Some(c.target_approach),
            CoolingTowerVisualState::Scalars(_) => None,
        }
    }

    /// The cooling range across the tower, or `None` when the cold-water
    /// temperature is not known (the physics-backed path).
    pub fn cooling_range(&self) -> Option<TemperatureInterval> {
        let s = self.scalars()?;
        Some(cooling_range(s.water_inlet_temp, s.water_outlet_temp))
    }

    /// Draw a small label, unless labels are switched off.
    fn tag(&self, painter: &Painter, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        painter.text(
            at,
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(8.5),
            LABEL,
        );
    }

    /// Draw a left-aligned readout line, unless labels are switched off.
    fn readout(&self, painter: &Painter, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        painter.text(
            at,
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(8.5),
            LABEL,
        );
    }
}

impl Widget for CoolingTowerVisual {
    /// Draws the cooling tower for [`CoolingTowerVisual::kind`], coloured by
    /// the shared [`crate::components::temperature_colour`] map, with the plume
    /// drawn from the exit air's relative humidity (see [`plume_opacity`]).
    ///
    /// The artwork letterboxes to the kind's native proportions inside the
    /// allocated box (see [`CoolingTowerKind::fit_native_aspect`]), so it never
    /// stretches. Anything with no honest source is drawn in neutral
    /// [`UNKNOWN_FLUID`] grey, and the plume is simply absent.
    fn ui(self, ui: &mut Ui) -> Response {
        let box_rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(box_rect, Sense::hover());
        let painter = ui.painter_at(box_rect);
        let rect = self.kind.fit_native_aspect(box_rect);
        let drawn = self.state.resolve(self.min_temp, self.max_temp);

        match self.kind {
            CoolingTowerKind::NaturalDraught => self.draw_natural_draught(&painter, rect, &drawn),
            CoolingTowerKind::InducedDraught => self.draw_induced_draught(&painter, rect, &drawn),
        }
        self.draw_readouts(&painter, rect, &drawn);

        response
    }
}

// ── Shared internals ────────────────────────────────────────────────────────

impl CoolingTowerVisual {
    /// Draws what is inside every cooling tower, whatever pulls the air
    /// through it: the hot-water distribution deck and its sprays, the fill
    /// pack, the rain zone, and the cold-water basin.
    ///
    /// `area` runs from the top of the distribution deck to the bottom of the
    /// basin. Bands within it, as fractions of its height: deck 0.00–0.10,
    /// fill 0.10–0.42, rain 0.42–0.72, basin 0.72–1.00.
    ///
    /// The falling water is graded from the hot inlet temperature at the deck
    /// to the cold outlet temperature at the basin — a display interpolation
    /// (see [`lerp_temperature`]), not a Merkel balance. With no water
    /// circulating, or with no water temperatures known, no spray and no rain
    /// are drawn at all.
    fn draw_internals(&self, painter: &Painter, area: Rect, drawn: &DrawnTower) {
        let w = area.width();
        let h = area.height();
        let y = |f: f32| area.top() + f * h;

        let deck_y = y(0.06);
        let fill_top = y(0.10);
        let fill_bottom = y(0.42);
        let rain_bottom = y(0.72);
        let basin = Rect::from_min_max(
            Pos2::new(area.left(), rain_bottom),
            Pos2::new(area.right(), area.bottom()),
        );

        // ── Hot-water distribution header and spray nozzles ────────────────
        let hot = drawn.colour(drawn.water_inlet_temp);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(area.left() + w * 0.06, deck_y - h * 0.022),
                Pos2::new(area.right() - w * 0.06, deck_y + h * 0.022),
            ),
            2.0,
            hot,
        );
        self.tag(
            painter,
            Pos2::new(area.center().x, deck_y - h * 0.06),
            "hot water distribution",
        );

        let circulating = drawn.is_circulating();
        let nozzles = 7;
        for k in 0..nozzles {
            let nx = area.left() + w * (0.10 + 0.80 * (k as f32 + 0.5) / nozzles as f32);
            painter.line_segment(
                [
                    Pos2::new(nx, deck_y + h * 0.022),
                    Pos2::new(nx, deck_y + h * 0.05),
                ],
                Stroke::new(1.4, hot),
            );
            if circulating {
                // A spray cone under each nozzle.
                painter.add(Shape::convex_polygon(
                    vec![
                        Pos2::new(nx, deck_y + h * 0.05),
                        Pos2::new(nx - w * 0.05, fill_top),
                        Pos2::new(nx + w * 0.05, fill_top),
                    ],
                    translucent(hot, 110),
                    Stroke::NONE,
                ));
            }
        }

        // ── Fill pack ──────────────────────────────────────────────────────
        //
        // The fill is what makes the tower work: it spreads the water into a
        // thin film over a large area so the air can evaporate some of it.
        let fill = Rect::from_min_max(
            Pos2::new(area.left() + w * 0.04, fill_top),
            Pos2::new(area.right() - w * 0.04, fill_bottom),
        );
        painter.rect_filled(fill, 2.0, translucent(INTERNALS, 220));
        let sheets = 11;
        for k in 0..sheets {
            let sx = fill.left() + fill.width() * (k as f32 + 0.5) / sheets as f32;
            painter.line_segment(
                [
                    Pos2::new(sx, fill.top() + h * 0.01),
                    Pos2::new(sx, fill.bottom() - h * 0.01),
                ],
                Stroke::new(1.1, translucent(OUTLINE, 120)),
            );
        }
        for k in 0..4 {
            let sy = fill.top() + fill.height() * (k as f32 + 0.5) / 4.0;
            painter.line_segment(
                [
                    Pos2::new(fill.left() + w * 0.01, sy),
                    Pos2::new(fill.right() - w * 0.01, sy),
                ],
                Stroke::new(0.9, translucent(OUTLINE, 70)),
            );
        }
        self.tag(painter, Pos2::new(area.center().x, y(0.26)), "fill pack");

        // ── Rain zone ──────────────────────────────────────────────────────
        let rain = Rect::from_min_max(
            Pos2::new(fill.left(), fill_bottom),
            Pos2::new(fill.right(), rain_bottom),
        );
        if circulating && rain.height() > 0.0 {
            for i in 0..34 {
                let p = scatter_point(rain, i, 101);
                let length = (h * 0.035) * (0.4 + tower_hash(i, 2, 107));
                let f = ((p.y - rain.top()) / rain.height()).clamp(0.0, 1.0);
                painter.line_segment(
                    [p, Pos2::new(p.x, (p.y + length).min(rain.bottom()))],
                    Stroke::new(1.3, translucent(drawn.falling_water_colour(f), 200)),
                );
            }
        } else {
            self.tag(
                painter,
                Pos2::new(area.center().x, y(0.57)),
                "no circulating flow",
            );
        }

        // ── Cold-water basin ───────────────────────────────────────────────
        painter.rect_filled(basin, radius(w * 0.02), CONCRETE);
        let pool = basin.shrink2(Vec2::new(w * 0.02, h * 0.02));
        painter.rect_filled(pool, radius(w * 0.012), VOID);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(pool.left(), pool.center().y), pool.max),
            radius(w * 0.012),
            translucent(drawn.colour(drawn.water_outlet_temp), 215),
        );
        self.tag(
            painter,
            Pos2::new(area.center().x, pool.center().y + h * 0.055),
            "cold-water basin",
        );
    }

    /// Draws an air arrow of half-height `size` at `at`, pointing right when
    /// `dx` is positive and left when it is negative.
    fn air_arrow(&self, painter: &Painter, at: Pos2, dx: f32, size: f32, colour: Color32) {
        painter.line_segment(
            [Pos2::new(at.x - dx, at.y), Pos2::new(at.x, at.y)],
            Stroke::new(1.6, colour),
        );
        painter.add(Shape::convex_polygon(
            vec![
                Pos2::new(at.x + dx.signum() * size * 1.4, at.y),
                Pos2::new(at.x, at.y - size),
                Pos2::new(at.x, at.y + size),
            ],
            colour,
            Stroke::NONE,
        ));
    }

    /// Draws an upward air arrow at `at` with half-width `size`.
    fn up_arrow(&self, painter: &Painter, at: Pos2, length: f32, size: f32, colour: Color32) {
        painter.line_segment(
            [Pos2::new(at.x, at.y + length), Pos2::new(at.x, at.y)],
            Stroke::new(1.6, colour),
        );
        painter.add(Shape::convex_polygon(
            vec![
                Pos2::new(at.x, at.y - size * 1.4),
                Pos2::new(at.x - size, at.y),
                Pos2::new(at.x + size, at.y),
            ],
            colour,
            Stroke::NONE,
        ));
    }

    /// Draws the exit plume above `mouth`, at the opacity
    /// [`plume_opacity`] gives for the exit air.
    ///
    /// Nothing is drawn at zero opacity — which includes every case where the
    /// exit air state is unknown, so a tower whose exit conditions have not
    /// been evaluated simply has no plume rather than a faint one.
    fn draw_plume(&self, painter: &Painter, mouth: Rect, opacity: f32) {
        if !(opacity > 0.0) {
            return;
        }
        let puffs = 16;
        for i in 0..puffs {
            let p = scatter_point(mouth, i, 113);
            // Puffs grow and fade as they rise out of the mouth.
            let rise = tower_hash(i, 3, 127);
            let r = mouth.width() * (0.10 + 0.16 * rise);
            let alpha = (opacity * (0.55 - 0.35 * rise) * 255.0).clamp(0.0, 255.0) as u8;
            painter.circle_filled(p, r, translucent(PLUME, alpha));
        }
    }

    /// Draws the numeric readouts under the tower: approach and range on the
    /// scalar path, the labelled target approach on the physics path, plus the
    /// inlet and exit air conditions and the circulating flow.
    ///
    /// Every line states where its number comes from. In particular a target is
    /// always printed with the word "target", and a quantity with no source
    /// prints "not evaluated" rather than being omitted silently — a reader
    /// must be able to tell "unknown" from "not shown".
    fn draw_readouts(&self, painter: &Painter, rect: Rect, drawn: &DrawnTower) {
        if !self.show_labels {
            return;
        }
        let x = rect.left() + rect.width() * 0.03;
        let mut line = rect.bottom() + rect.height() * 0.035;
        let step = rect.height() * 0.036;

        let mut put = |text: String| {
            self.readout(painter, Pos2::new(x, line), &text);
            line += step;
        };

        match (self.approach(), self.target_approach()) {
            (Some(approach), _) => {
                let k = approach.get::<kelvin_interval>();
                if k > 0.0 {
                    put(format!("approach to wet bulb  {k:.1} K"));
                } else {
                    put(format!("approach to wet bulb  {k:.1} K  (<= 0: check model)"));
                }
            }
            (None, Some(target)) => put(format!(
                "target approach  {:.1} K  (set-point, not achieved)",
                target.get::<kelvin_interval>()
            )),
            (None, None) => put("approach: not evaluated".to_string()),
        }

        match self.cooling_range() {
            Some(range) => put(format!(
                "range  {:.1} K",
                range.get::<kelvin_interval>()
            )),
            None => put("range: not evaluated".to_string()),
        }

        if let Some(air) = drawn.air_inlet {
            put(format!(
                "air in  {:.1} degC  RH {:.0} %  W {:.4} kg/kg",
                air.t_dry_bulb.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
                air.relative_humidity.get::<percent>(),
                air.humidity_ratio.get::<ratio>()
            ));
        }
        match drawn.air_outlet {
            Some(air) => put(format!(
                "air out {:.1} degC  RH {:.0} %",
                air.t_dry_bulb.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
                air.relative_humidity.get::<percent>()
            )),
            None => put("air out: not evaluated — no plume drawn".to_string()),
        }
        if let Some(q) = drawn.water_flow_rate {
            put(format!(
                "circulating water  {:.2} m^3/s",
                q.get::<cubic_meter_per_second>()
            ));
        }
    }
}

// ── 1. Natural draught (hyperbolic shell) ───────────────────────────────────

impl CoolingTowerVisual {
    /// Draws the natural-draught hyperbolic tower.
    ///
    /// Bottom to top: the cold-water basin at ground level; the annular air
    /// inlet under the shell, carried on raked columns; the fill pack and its
    /// distribution deck; then the empty chimney, whose whole job is to hold a
    /// tall column of warm moist air so that buoyancy pulls fresh air in at the
    /// bottom. The shell profile is the meridian hyperbola of a hyperboloid of
    /// one sheet — see [`hyperboloid_half_width`].
    ///
    /// Illustrative geometry: the shape is the real curve, the proportions are
    /// chosen for legibility.
    fn draw_natural_draught(&self, painter: &Painter, rect: Rect, drawn: &DrawnTower) {
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let y = |f: f32| rect.top() + f * h;

        // Scale the throat so the widest point of the shell (the base) exactly
        // fills the drawn width.
        let base_shape = hyperboloid_half_width(1.0, HYPERBOLOID_THROAT_FRACTION, 1.0, HYPERBOLOID_FLARE);
        let throat_half = 0.5 * w / base_shape;
        let half = |f: f32| {
            hyperboloid_half_width(f, HYPERBOLOID_THROAT_FRACTION, throat_half, HYPERBOLOID_FLARE)
        };

        let shell_bottom = 0.955f32;
        let samples = 40;

        // ── Shell ──────────────────────────────────────────────────────────
        //
        // Drawn as a stack of horizontal bands rather than as one polygon: a
        // hyperboloid silhouette is **concave** at the waist, and egui's
        // convex-polygon fill would cut the waist off. Each band spans two
        // adjacent samples of the meridian and is therefore a trapezium, which
        // is convex and tessellates correctly.
        let thickness = w * 0.018;
        for k in 0..samples {
            let f0 = shell_bottom * k as f32 / samples as f32;
            let f1 = shell_bottom * (k + 1) as f32 / samples as f32;
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(cx - half(f0), y(f0)),
                    Pos2::new(cx + half(f0), y(f0)),
                    Pos2::new(cx + half(f1), y(f1)),
                    Pos2::new(cx - half(f1), y(f1)),
                ],
                CONCRETE,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(cx - half(f0) + thickness, y(f0)),
                    Pos2::new(cx + half(f0) - thickness, y(f0)),
                    Pos2::new(cx + half(f1) - thickness, y(f1)),
                    Pos2::new(cx - half(f1) + thickness, y(f1)),
                ],
                VOID,
                Stroke::NONE,
            ));
        }

        // The two meridians, as lines — a line shape has no convexity
        // requirement, so the waist survives.
        let mut left_edge = Vec::with_capacity(samples + 1);
        let mut right_edge = Vec::with_capacity(samples + 1);
        for k in 0..=samples {
            let f = shell_bottom * k as f32 / samples as f32;
            left_edge.push(Pos2::new(cx - half(f), y(f)));
            right_edge.push(Pos2::new(cx + half(f), y(f)));
        }

        // Shell meridian lines, which also make the waist read as a waist.
        for k in 0..=samples {
            let f = shell_bottom * k as f32 / samples as f32;
            if k % 4 != 0 {
                continue;
            }
            painter.line_segment(
                [
                    Pos2::new(cx - half(f), y(f)),
                    Pos2::new(cx - half(f) + thickness, y(f)),
                ],
                Stroke::new(1.0, translucent(OUTLINE, 90)),
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.16)), "throat");

        // ── Internals, standing in the base of the shell ───────────────────
        let internals = Rect::from_min_max(
            Pos2::new(cx - half(0.90) * 0.86, y(0.72)),
            Pos2::new(cx + half(0.90) * 0.86, y(0.97)),
        );
        self.draw_internals(painter, internals, drawn);

        // ── Air inlet, under the shell on raked columns ────────────────────
        let air_in = drawn.colour(drawn.air_inlet.map(|a| a.t_dry_bulb));
        for side in [-1.0f32, 1.0] {
            let outer = cx + side * half(0.985);
            let inner_x = cx + side * half(0.985) * 0.72;
            // Raked columns carrying the shell over the air inlet.
            for k in 0..4 {
                let t0 = k as f32 / 4.0;
                let t1 = (k as f32 + 1.0) / 4.0;
                painter.line_segment(
                    [
                        Pos2::new(outer + (inner_x - outer) * t0, y(0.955)),
                        Pos2::new(outer + (inner_x - outer) * t1 * 0.35, y(0.995)),
                    ],
                    Stroke::new((w * 0.012).max(1.2), CONCRETE),
                );
            }
            self.air_arrow(
                painter,
                Pos2::new(cx + side * half(0.90) * 0.92, y(0.905)),
                -side * w * 0.13,
                h * 0.012,
                air_in,
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.995)), "air in — no fan");

        // ── Exit air and plume ─────────────────────────────────────────────
        //
        // The buoyant column inside the shell is the draught, so the exit
        // arrows are drawn inside the throat rather than at a nozzle.
        match drawn.air_outlet {
            Some(air) => {
                let colour = drawn.colour(Some(air.t_dry_bulb));
                for side in [-0.45f32, 0.0, 0.45] {
                    self.up_arrow(
                        painter,
                        Pos2::new(cx + side * half(0.16), y(0.05)),
                        h * 0.10,
                        w * 0.018,
                        colour,
                    );
                }
            }
            None => {
                self.tag(painter, Pos2::new(cx, y(0.075)), "exit air not evaluated");
            }
        }
        let mouth = Rect::from_min_max(
            Pos2::new(cx - half(0.0) * 0.9, rect.top() - h * 0.10),
            Pos2::new(cx + half(0.0) * 0.9, rect.top() + h * 0.01),
        );
        self.draw_plume(painter, mouth, drawn.plume());

        // Silhouette last, so the waist reads on top.
        painter.add(Shape::line(left_edge, Stroke::new(1.4, OUTLINE)));
        painter.add(Shape::line(right_edge, Stroke::new(1.4, OUTLINE)));
    }
}

// ── 2. Mechanical induced draught (fan cell) ────────────────────────────────

impl CoolingTowerVisual {
    /// Draws the mechanical induced-draught cell.
    ///
    /// A box with air-inlet louvres down each side, the same internals as the
    /// natural-draught tower, drift eliminators above the distribution deck,
    /// and a fan in a velocity-recovery stack on the roof. The fan is
    /// **induced** draught — it sits downstream of the fill and works in the
    /// warm wet air leaving it, which is why it is above the eliminators and
    /// not under the fill.
    ///
    /// The fan turns at `theta = omega * t` from the caller-supplied speed and
    /// the application's clock; with no speed supplied it is drawn complete but
    /// stationary (see the module documentation).
    fn draw_induced_draught(&self, painter: &Painter, rect: Rect, drawn: &DrawnTower) {
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let x = |f: f32| rect.left() + f * w;
        let y = |f: f32| rect.top() + f * h;

        let casing = Rect::from_min_max(Pos2::new(x(0.06), y(0.28)), Pos2::new(x(0.94), y(0.92)));
        let stack = Rect::from_min_max(Pos2::new(x(0.36), y(0.08)), Pos2::new(x(0.64), y(0.28)));

        // ── Fan stack, casing and interior ─────────────────────────────────
        painter.rect_filled(stack, radius(w * 0.01), STEEL);
        painter.rect_filled(casing, radius(w * 0.008), STEEL);
        let interior = casing.shrink(w * 0.012);
        painter.rect_filled(interior, radius(w * 0.006), VOID);
        painter.rect_filled(stack.shrink(w * 0.014), 0.0, VOID);

        // ── Fan, on the roof and downstream of the fill ────────────────────
        let fan_y = y(0.155);
        let fan_half = w * 0.125;
        let theta = self
            .fan_angle()
            .map(|a| a.get::<uom::si::angle::radian>() as f32)
            .unwrap_or(0.0);
        painter.line_segment(
            [
                Pos2::new(cx - fan_half, fan_y),
                Pos2::new(cx + fan_half, fan_y),
            ],
            Stroke::new(1.0, translucent(INTERNALS, 160)),
        );
        let blades = 4;
        for b in 0..blades {
            let angle = theta + b as f32 * TAU / blades as f32;
            // Seen from the side, a blade running radially from the hub
            // projects as its own length times cos(angle) — so it shortens onto
            // the hub as it turns edge-on instead of floating free.
            let tip_x = cx + fan_half * angle.cos();
            let tip_y = fan_y + h * 0.018 * angle.sin();
            painter.line_segment(
                [Pos2::new(cx, fan_y), Pos2::new(tip_x, tip_y)],
                Stroke::new((h * 0.016).max(1.6), INTERNALS),
            );
        }
        painter.circle_filled(Pos2::new(cx, fan_y), (w * 0.022).max(2.0), INTERNALS);
        self.tag(painter, Pos2::new(cx, y(0.055)), "induced-draught fan");

        // ── Drift eliminators, just under the fan deck ─────────────────────
        let eliminator_top = y(0.30);
        let eliminator_bottom = y(0.35);
        for k in 0..9 {
            let x0 = interior.left() + interior.width() * (k as f32 + 0.1) / 9.0;
            let x1 = interior.left() + interior.width() * (k as f32 + 0.5) / 9.0;
            let x2 = interior.left() + interior.width() * (k as f32 + 0.9) / 9.0;
            painter.line_segment(
                [
                    Pos2::new(x0, eliminator_bottom),
                    Pos2::new(x1, eliminator_top),
                ],
                Stroke::new(1.2, INTERNALS),
            );
            painter.line_segment(
                [
                    Pos2::new(x1, eliminator_top),
                    Pos2::new(x2, eliminator_bottom),
                ],
                Stroke::new(1.2, INTERNALS),
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.325)), "drift eliminators");

        // ── Internals ──────────────────────────────────────────────────────
        let internals = Rect::from_min_max(
            Pos2::new(interior.left() + w * 0.02, y(0.38)),
            Pos2::new(interior.right() - w * 0.02, y(0.90)),
        );
        self.draw_internals(painter, internals, drawn);

        // ── Air-inlet louvres down each side ───────────────────────────────
        let air_in = drawn.colour(drawn.air_inlet.map(|a| a.t_dry_bulb));
        for side in [-1.0f32, 1.0] {
            let edge = cx + side * (casing.width() * 0.5 - w * 0.004);
            for k in 0..6 {
                let ly = y(0.60) + k as f32 * h * 0.045;
                painter.line_segment(
                    [
                        Pos2::new(edge, ly),
                        Pos2::new(edge - side * w * 0.035, ly + h * 0.025),
                    ],
                    Stroke::new(1.4, INTERNALS),
                );
            }
            self.air_arrow(
                painter,
                Pos2::new(edge - side * w * 0.055, y(0.70)),
                -side * w * 0.09,
                h * 0.016,
                air_in,
            );
        }
        self.tag(painter, Pos2::new(cx, y(0.945)), "air in through louvres");

        // ── Exit air and plume ─────────────────────────────────────────────
        match drawn.air_outlet {
            Some(air) => {
                let colour = drawn.colour(Some(air.t_dry_bulb));
                for side in [-0.4f32, 0.4] {
                    self.up_arrow(
                        painter,
                        Pos2::new(cx + side * fan_half, y(0.10)),
                        h * 0.05,
                        w * 0.016,
                        colour,
                    );
                }
            }
            None => {
                self.tag(painter, Pos2::new(cx, y(0.10)), "exit air not evaluated");
            }
        }
        let mouth = Rect::from_min_max(
            Pos2::new(stack.left(), rect.top() - h * 0.09),
            Pos2::new(stack.right(), rect.top() + h * 0.02),
        );
        self.draw_plume(painter, mouth, drawn.plume());

        painter.rect_stroke(
            casing,
            radius(w * 0.008),
            Stroke::new(1.4, OUTLINE),
            StrokeKind::Middle,
        );
        painter.rect_stroke(
            stack,
            radius(w * 0.01),
            Stroke::new(1.4, OUTLINE),
            StrokeKind::Middle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines::humid_air::state_from_t_p_r;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::f64::Pressure;
    use uom::si::pressure::kilopascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;
    use uom::si::volume_rate::cubic_meter_per_second;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    /// A CoolProp-resolved humid-air state at 101.325 kPa.
    fn air(t_degc: f64, rh: f64) -> HumidAirState {
        state_from_t_p_r(
            degc(t_degc),
            Pressure::new::<kilopascal>(101.325),
            Ratio::new::<ratio>(rh),
        )
        .expect("humid-air state must resolve on the liquid-water branch")
    }

    /// Illustrative tower state: 32 degC / 60 % ambient air in, saturated air
    /// out at 38 degC, water cooled 40 -> 30 degC against a 25.4 degC wet
    /// bulb. Demonstration values only, from no design.
    fn scalars() -> CoolingTowerScalars {
        CoolingTowerScalars {
            air_inlet: air(32.0, 0.60),
            air_outlet: air(38.0, 1.00),
            inlet_wet_bulb: degc(25.4),
            water_inlet_temp: degc(40.0),
            water_outlet_temp: degc(30.0),
            water_flow_rate: VolumeRate::new::<cubic_meter_per_second>(12.0),
        }
    }

    fn visual(kind: CoolingTowerKind) -> CoolingTowerVisual {
        CoolingTowerVisual::from_scalars(
            kind,
            Pos2::new(100.0, 200.0),
            Vec2::new(300.0, 400.0),
            degc(0.0),
            degc(100.0),
            scalars(),
        )
    }

    fn physics_tower() -> CoolingTower {
        CoolingTower::new(
            air(32.0, 0.60),
            degc(40.0),
            VolumeRate::new::<cubic_meter_per_second>(12.0),
            TemperatureInterval::new::<kelvin_interval>(4.0),
        )
    }

    /// Each architecture must keep its own proportions at any box size.
    ///
    /// **Methodology.** A natural-draught shell is tall because the chimney
    /// height *is* the draught, and a fan cell is squat because it has none, so
    /// letting either stretch to fill its box would erase the distinction.
    /// Require the two [`CoolingTowerKind::native_aspect_ratio`] values to
    /// straddle 1.0, and require [`CoolingTowerKind::fit_native_aspect`] to
    /// preserve the ratio, stay centred and never overflow, in a square box, an
    /// over-wide box and an over-tall box.
    ///
    /// **Result (2026-08-12):** ratios 0.72 (natural draught) and 1.45
    /// (induced draught); all six box/kind combinations preserved the ratio to
    /// better than 1e-4 and stayed centred to better than 1e-4 points, with the
    /// fitted rectangle never exceeding its box. Interpretation: the hyperbolic
    /// shell stays tall in a wide card and the fan cell stays squat in a tall
    /// one, so the two read as different machines at any gallery size.
    #[test]
    fn each_kind_letterboxes_to_its_own_proportions() {
        assert!(CoolingTowerKind::NaturalDraught.native_aspect_ratio() < 1.0);
        assert!(CoolingTowerKind::InducedDraught.native_aspect_ratio() > 1.0);

        for kind in CoolingTowerKind::ALL {
            for size in [
                Vec2::new(300.0, 300.0),
                Vec2::new(900.0, 200.0),
                Vec2::new(100.0, 900.0),
            ] {
                let box_rect = Rect::from_min_size(Pos2::new(17.0, 23.0), size);
                let fitted = kind.fit_native_aspect(box_rect);
                println!(
                    "{:?} in {size:?} -> {:.1}x{:.1}",
                    kind,
                    fitted.width(),
                    fitted.height()
                );
                assert!(
                    (fitted.width() / fitted.height() - kind.native_aspect_ratio()).abs() < 1e-4,
                    "{kind:?} in box {size:?} did not preserve its ratio"
                );
                assert!(
                    (fitted.center() - box_rect.center()).length() < 1e-4,
                    "{kind:?} in box {size:?} was not centred"
                );
                assert!(
                    fitted.width() <= size.x + 1e-3 && fitted.height() <= size.y + 1e-3,
                    "{kind:?} in box {size:?} overflowed its box"
                );
            }
        }
    }

    /// A degenerate box must not produce NaN geometry — zero-height
    /// allocations happen transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        for kind in CoolingTowerKind::ALL {
            assert_eq!(kind.fit_native_aspect(flat), flat);
        }
    }

    /// The shell must be a real hyperboloid meridian, with its waist at the
    /// throat.
    ///
    /// **Methodology.** A natural-draught shell is a hyperboloid of one sheet,
    /// so its silhouette is the hyperbola
    /// `x = a * sqrt(1 + ((f - f_throat) / b)^2)`. Require
    /// [`hyperboloid_half_width`] to take its minimum exactly at the throat and
    /// to equal the throat half-width there; to be symmetric in the distance
    /// from the throat; to increase strictly with that distance; to scale
    /// proportionally with the throat half-width so the shape survives
    /// resizing; and to degenerate to a cylinder for a non-positive flare
    /// rather than dividing by zero. Swept over 2 001 height fractions from
    /// -1.0 to 1.0.
    ///
    /// **Result (2026-08-12):** minimum 1.000000 at f = 0.16, matching the
    /// throat exactly; symmetric to better than 1e-6 at every tested offset;
    /// strictly increasing away from the throat at every step; with the drawn
    /// parameters (throat fraction 0.16, flare 0.80) the base at f = 1.0 is
    /// 1.4500 times the throat width, which is the waist a cooling tower reads
    /// by; a zero flare gave a constant width. Interpretation: the silhouette
    /// is generated by the curve that defines the surface, not sketched.
    #[test]
    fn the_shell_is_a_hyperboloid_with_its_waist_at_the_throat() {
        let a = 1.0f32;
        let at_throat = hyperboloid_half_width(
            HYPERBOLOID_THROAT_FRACTION,
            HYPERBOLOID_THROAT_FRACTION,
            a,
            HYPERBOLOID_FLARE,
        );
        assert!((at_throat - a).abs() < 1e-6);

        let mut minimum = f32::INFINITY;
        let mut minimum_at = f32::NAN;
        let mut samples = 0usize;
        for k in -1000..=1000 {
            let f = k as f32 * 0.001;
            let width =
                hyperboloid_half_width(f, HYPERBOLOID_THROAT_FRACTION, a, HYPERBOLOID_FLARE);
            assert!(width >= a - 1e-6, "the shell narrowed past its throat");
            if width < minimum {
                minimum = width;
                minimum_at = f;
            }
            samples += 1;
        }
        println!("{samples} samples; minimum {minimum:.6} at f = {minimum_at:.3}");
        assert!((minimum_at - HYPERBOLOID_THROAT_FRACTION).abs() < 2e-3);

        // Symmetric about the throat, and strictly increasing away from it.
        let mut previous = at_throat;
        for k in 1..=200 {
            let d = k as f32 * 0.004;
            let below =
                hyperboloid_half_width(HYPERBOLOID_THROAT_FRACTION - d, HYPERBOLOID_THROAT_FRACTION, a, HYPERBOLOID_FLARE);
            let above =
                hyperboloid_half_width(HYPERBOLOID_THROAT_FRACTION + d, HYPERBOLOID_THROAT_FRACTION, a, HYPERBOLOID_FLARE);
            assert!((below - above).abs() < 1e-6, "the shell is not symmetric");
            assert!(above > previous, "the shell did not flare monotonically");
            previous = above;
        }

        let base = hyperboloid_half_width(1.0, HYPERBOLOID_THROAT_FRACTION, a, HYPERBOLOID_FLARE);
        println!("base is {base:.4} times the throat half-width");
        assert!((base - 1.45).abs() < 0.02, "base/throat was {base}");

        // Proportional in the throat width, so the shape survives resizing.
        assert!(
            (hyperboloid_half_width(0.8, HYPERBOLOID_THROAT_FRACTION, 2.0, HYPERBOLOID_FLARE)
                - 2.0 * hyperboloid_half_width(0.8, HYPERBOLOID_THROAT_FRACTION, 1.0, HYPERBOLOID_FLARE))
            .abs()
                < 1e-5
        );
        // A degenerate flare must not divide by zero.
        assert_eq!(hyperboloid_half_width(0.5, 0.16, 7.0, 0.0), 7.0);
        assert_eq!(hyperboloid_half_width(0.5, 0.16, 7.0, -1.0), 7.0);
    }

    /// The plume must be driven by how far the exit air sits into saturation,
    /// and by nothing else.
    ///
    /// **Methodology.** A cooling-tower plume is condensed water, so it needs
    /// the exit air at or very near saturation. Require [`plume_opacity`] to be
    /// zero at and below [`PLUME_VISIBLE_RH_MIN`], to rise monotonically from
    /// there, to reach exactly 1.0 at saturation, to saturate rather than grow
    /// beyond `R = 1` (a `HumidAirState` cannot describe air already carrying
    /// droplets), and to draw nothing for a non-finite relative humidity.
    /// Swept over 201 relative humidities from -0.5 to 1.5.
    ///
    /// **Result (2026-08-12):** 201 samples, every value inside `[0, 1]` and
    /// non-decreasing; opacity 0.0 for all `R <= 0.90`, 0.50 at `R = 0.95`,
    /// 1.0 at `R = 1.00` and still 1.0 at `R = 1.5`; NaN, +inf and -inf all
    /// gave 0.0. Interpretation: unsaturated exit air carries its water
    /// invisibly, as it should, and no NaN can produce a confident-looking
    /// cloud.
    #[test]
    fn the_plume_tracks_exit_saturation_only() {
        let rh = |v: f32| Ratio::new::<ratio>(v as f64);
        let mut previous = 0.0f32;
        let mut samples = 0usize;
        for k in -50..=150 {
            let r = k as f32 * 0.01;
            let opacity = plume_opacity(rh(r));
            assert!(
                (0.0..=1.0).contains(&opacity),
                "opacity {opacity} out of range at R = {r}"
            );
            assert!(opacity >= previous - 1e-6, "opacity fell at R = {r}");
            if r <= PLUME_VISIBLE_RH_MIN {
                assert_eq!(opacity, 0.0, "a plume appeared at R = {r}");
            }
            previous = opacity;
            samples += 1;
        }
        println!("{samples} relative humidities checked");

        assert_eq!(plume_opacity(rh(PLUME_VISIBLE_RH_MIN)), 0.0);
        assert!((plume_opacity(rh(0.95)) - 0.5).abs() < 1e-5);
        assert_eq!(plume_opacity(rh(1.0)), 1.0);
        assert_eq!(plume_opacity(rh(1.5)), 1.0);
        assert_eq!(plume_opacity(rh(f32::NAN)), 0.0);
        assert_eq!(plume_opacity(rh(f32::INFINITY)), 0.0);
        assert_eq!(plume_opacity(rh(f32::NEG_INFINITY)), 0.0);
    }

    /// The approach and the range must be the real differences of the caller's
    /// temperatures, in kelvin, with the physical sign convention.
    ///
    /// **Methodology.** Approach is `T_water,out - T_wb` and range is
    /// `T_water,in - T_water,out`; both are temperature *intervals*, not
    /// temperatures. Check them against hand-computed values for the
    /// illustrative state (40 -> 30 degC water against a 25.4 degC wet bulb),
    /// require a physically impossible sub-wet-bulb outlet to report a negative
    /// approach rather than being clamped away, and require the identity
    /// `T_water,in - T_wb = range + approach` to hold.
    ///
    /// **Result (2026-08-12):** approach 4.600 K and range 10.000 K for the
    /// illustrative state; an outlet 1 K below the wet bulb gave -1.000 K; the
    /// identity held to better than 1e-9 K. Interpretation: the readouts are
    /// arithmetic on the caller's own state, and an out-of-range model shows
    /// up on screen instead of being hidden.
    #[test]
    fn the_approach_and_range_are_real_temperature_differences() {
        let s = scalars();
        let approach = approach_to_wet_bulb(s.water_outlet_temp, s.inlet_wet_bulb);
        let range = cooling_range(s.water_inlet_temp, s.water_outlet_temp);
        println!(
            "approach {:.3} K, range {:.3} K",
            approach.get::<kelvin_interval>(),
            range.get::<kelvin_interval>()
        );
        assert!((approach.get::<kelvin_interval>() - 4.6).abs() < 1e-9);
        assert!((range.get::<kelvin_interval>() - 10.0).abs() < 1e-9);

        // A tower cannot cool below the wet bulb; if a model says it did, show
        // the negative number rather than hiding it.
        let impossible = approach_to_wet_bulb(degc(24.4), degc(25.4));
        assert!((impossible.get::<kelvin_interval>() + 1.0).abs() < 1e-9);

        let total = s.water_inlet_temp.get::<kelvin>() - s.inlet_wet_bulb.get::<kelvin>();
        assert!(
            (total - (range.get::<kelvin_interval>() + approach.get::<kelvin_interval>())).abs()
                < 1e-9
        );

        // And the widget must report the same numbers as the free functions.
        let v = visual(CoolingTowerKind::NaturalDraught);
        assert_eq!(v.approach(), Some(approach));
        assert_eq!(v.cooling_range(), Some(range));
    }

    /// The physics-backed path must draw what the component really holds and
    /// **nothing** downstream of it.
    ///
    /// **Methodology.** `tampines::components::CoolingTower` holds a real
    /// CoolProp-resolved air inlet state, a real water inlet temperature and
    /// flow rate, and a *target* approach; its `evaluate` returns
    /// `NotYetImplemented`, so there is no exit air state and no water outlet
    /// temperature. Wrap one with [`CoolingTowerVisual::new`] — the preserved
    /// five-argument signature — and require: the air inlet, water inlet
    /// temperature and flow to survive into the drawing state; the exit air,
    /// wet bulb and water outlet to be `None`; the plume opacity to be exactly
    /// zero, so no plume is drawn; the achieved approach and the range to be
    /// `None` while the target approach is reported separately; and the
    /// unknown cold-water colour to be neutral grey rather than a point on the
    /// temperature scale.
    ///
    /// **Result (2026-08-12):** the wrapped 32 degC / 60 % RH inlet air and
    /// 40 degC / 12 m^3/s water survived unchanged; exit air, wet bulb and
    /// water outlet were `None`; plume opacity 0.0; `approach()` and
    /// `cooling_range()` were `None` while `target_approach()` returned 4.0 K;
    /// the basin colour was `Color32::GRAY` (160, 160, 160). Interpretation:
    /// the physics path cannot grow a plume or an approach out of nothing, and
    /// a future change that starts inventing an ambient condition fails here.
    #[test]
    fn the_physics_path_draws_no_plume_and_no_approach() {
        let tower = physics_tower();
        let visual = CoolingTowerVisual::new(
            tower,
            Pos2::new(0.0, 0.0),
            Vec2::new(300.0, 400.0),
            degc(0.0),
            degc(100.0),
        );
        let drawn = visual.state.resolve(visual.min_temp, visual.max_temp);

        assert_eq!(drawn.air_inlet, Some(tower.air_inlet));
        assert_eq!(drawn.water_inlet_temp, Some(tower.water_inlet_temperature));
        assert_eq!(drawn.water_flow_rate, Some(tower.water_flow_rate));
        assert!(drawn.is_circulating());

        assert_eq!(drawn.air_outlet, None);
        assert_eq!(drawn.inlet_wet_bulb, None);
        assert_eq!(drawn.water_outlet_temp, None);
        assert_eq!(drawn.plume(), 0.0, "no exit air state means no plume");
        assert_eq!(drawn.colour(drawn.water_outlet_temp), UNKNOWN_FLUID);
        assert_eq!(drawn.falling_water_colour(0.5), UNKNOWN_FLUID);

        assert_eq!(visual.approach(), None);
        assert_eq!(visual.cooling_range(), None);
        assert_eq!(
            visual.target_approach(),
            Some(TemperatureInterval::new::<kelvin_interval>(4.0))
        );
        assert!(visual.scalars().is_none());
        assert_eq!(visual.physics(), Some(tower));
        assert_eq!(visual.kind(), CoolingTowerKind::NaturalDraught);
        assert_eq!(visual.size(), Vec2::new(300.0, 400.0));
    }

    /// The scalar path must pass the caller's state through untouched — this is
    /// real model state, not a placeholder, so nothing may be substituted.
    #[test]
    fn the_scalar_path_passes_state_through_unchanged() {
        for kind in CoolingTowerKind::ALL {
            let v = visual(*kind);
            assert_eq!(v.scalars(), Some(scalars()));
            assert_eq!(v.kind(), *kind);
            assert_eq!(v.size(), Vec2::new(300.0, 400.0));
            assert!(v.physics().is_none());
            assert!(v.target_approach().is_none());
        }
    }

    /// The scalar path must colour the air and water circuits from the caller's
    /// CoolProp-resolved states, and must draw a plume for saturated exit air.
    ///
    /// **Methodology.** Resolve the drawing state from the illustrative
    /// scalars and require: the inlet and exit air dry-bulb colours to be the
    /// mapped supplied temperatures and to differ from each other and from
    /// neutral grey; the falling-water colour to run from the mapped hot inlet
    /// at the deck to the mapped cold outlet at the basin; and the plume
    /// opacity to be full, because the exit air was supplied at saturation.
    ///
    /// **Result (2026-08-12):** the 32 degC inlet air and 38 degC exit air
    /// resolved to different, non-grey colours; the falling water matched the
    /// 40 degC mapping at `f = 0`, the 30 degC mapping at `f = 1` and the
    /// 35 degC mapping at `f = 0.5`; plume opacity 1.0 at the supplied
    /// `R = 1.00`. Interpretation: every painted region on this path traces to
    /// a number the caller supplied.
    #[test]
    fn the_scalar_path_colours_from_the_supplied_psychrometric_states() {
        let v = visual(CoolingTowerKind::InducedDraught);
        let drawn = v.state.resolve(v.min_temp, v.max_temp);
        let (lo, hi) = (degc(0.0), degc(100.0));

        let inlet_air = drawn.colour(drawn.air_inlet.map(|a| a.t_dry_bulb));
        let exit_air = drawn.colour(drawn.air_outlet.map(|a| a.t_dry_bulb));
        assert_eq!(inlet_air, temperature_colour(degc(32.0), lo, hi));
        assert_eq!(exit_air, temperature_colour(degc(38.0), lo, hi));
        assert_ne!(inlet_air, UNKNOWN_FLUID);
        assert_ne!(inlet_air, exit_air, "the air must visibly warm up");

        assert_eq!(
            drawn.falling_water_colour(0.0),
            temperature_colour(degc(40.0), lo, hi)
        );
        assert_eq!(
            drawn.falling_water_colour(1.0),
            temperature_colour(degc(30.0), lo, hi)
        );
        assert_eq!(
            drawn.falling_water_colour(0.5),
            temperature_colour(degc(35.0), lo, hi)
        );

        assert_eq!(drawn.plume(), 1.0, "saturated exit air must plume fully");
        println!(
            "inlet air {inlet_air:?}, exit air {exit_air:?}, plume {:.2}",
            drawn.plume()
        );
    }

    /// The supplied air states must really be CoolProp-resolved psychrometry,
    /// not free-floating numbers.
    ///
    /// **Methodology.** `tampines::humid_air::state_from_t_p_r` wraps
    /// `outram_park_fork_coolprop::humid_air::ha_props` (ASHRAE RP-1485).
    /// Resolve 32 degC / 60 % RH at 101.325 kPa and require the state to come
    /// back with the requested dry-bulb and relative humidity, a physically
    /// sensible humidity ratio, and a saturated state at the same temperature
    /// to carry strictly more water and more enthalpy.
    ///
    /// **Result (2026-08-12):** 32.000 degC at RH 0.600 gave W = 0.018121
    /// kg/kg and h = 78.56 kJ/kg dry air; the saturated state at the same
    /// temperature gave W = 0.030800 kg/kg and h = 110.98 kJ/kg dry air.
    /// Interpretation: the widget is reading a real psychrometric state, so
    /// the plume and the air colours are anchored to the CoolProp port rather
    /// than to display constants.
    #[test]
    fn the_air_states_come_from_the_coolprop_psychrometrics() {
        let humid = air(32.0, 0.60);
        assert!((humid.t_dry_bulb.get::<degree_celsius>() - 32.0).abs() < 1e-6);
        assert!((humid.relative_humidity.get::<ratio>() - 0.60).abs() < 1e-6);
        assert!(humid.humidity_ratio.get::<ratio>() > 0.0);

        let saturated = air(32.0, 1.00);
        println!(
            "32 degC: W = {:.6} at RH 0.60, W = {:.6} at RH 1.00",
            humid.humidity_ratio.get::<ratio>(),
            saturated.humidity_ratio.get::<ratio>()
        );
        println!(
            "enthalpy {:.2} -> {:.2} kJ/kg dry air",
            humid.enthalpy.get::<uom::si::available_energy::joule_per_kilogram>() / 1000.0,
            saturated.enthalpy.get::<uom::si::available_energy::joule_per_kilogram>() / 1000.0
        );
        assert!(
            saturated.humidity_ratio > humid.humidity_ratio,
            "saturated air must carry more water"
        );
        assert!(
            saturated.enthalpy > humid.enthalpy,
            "saturated air must carry more enthalpy"
        );
        assert_eq!(plume_opacity(saturated.relative_humidity), 1.0);
        assert_eq!(plume_opacity(humid.relative_humidity), 0.0);
    }

    /// A tower with nothing circulating must not be drawn with water falling
    /// through it, and a natural-draught tower must have no fan.
    #[test]
    fn a_stopped_tower_is_drawn_stopped() {
        let mut s = scalars();
        s.water_flow_rate = VolumeRate::ZERO;
        let v = CoolingTowerVisual::from_scalars(
            CoolingTowerKind::InducedDraught,
            Pos2::ZERO,
            Vec2::new(300.0, 400.0),
            degc(0.0),
            degc(100.0),
            s,
        );
        let drawn = v.state.resolve(v.min_temp, v.max_temp);
        assert!(!drawn.is_circulating(), "zero flow must draw no rain");
        // The air side is unaffected: the fan can still be running.
        assert_eq!(drawn.plume(), 1.0);

        assert!(!CoolingTowerKind::NaturalDraught.has_fan());
        assert!(CoolingTowerKind::InducedDraught.has_fan());
    }

    /// The fan phase must be the physical product `theta = omega * t`, not a
    /// tuned animation rate — and a tower with no fan must report none.
    ///
    /// **Methodology.** `uom`'s type algebra makes an angular velocity times a
    /// time an angle exactly. Require a 3 rad/s fan at 10 s to be at 30 rad, a
    /// zero speed to give exactly zero phase at any time, a negative speed to
    /// run backwards, and [`CoolingTowerKind::NaturalDraught`] to return
    /// `None` whatever speed it is given.
    ///
    /// **Result (2026-08-12):** 30.000000 rad at 3 rad/s and 10 s; 0.0 rad at
    /// zero speed and 1 000 s; -15.0 rad at -3 rad/s and 5 s; the
    /// natural-draught tower returned `None` at every speed. Interpretation:
    /// the fan turns at the speed it is told and a fanless tower cannot be made
    /// to grow one.
    #[test]
    fn the_fan_angle_is_omega_times_time() {
        let fan = |omega: f64, t: f64| {
            visual(CoolingTowerKind::InducedDraught)
                .with_fan_speed(AngularVelocity::new::<radian_per_second>(omega))
                .at_time(Time::new::<second>(t))
                .fan_angle()
                .map(|a| a.get::<uom::si::angle::radian>())
        };
        assert!((fan(3.0, 10.0).unwrap() - 30.0).abs() < 1e-9);
        assert_eq!(fan(0.0, 1000.0), Some(0.0));
        assert!((fan(-3.0, 5.0).unwrap() + 15.0).abs() < 1e-9);

        for omega in [0.0, 3.0, -3.0] {
            assert_eq!(
                visual(CoolingTowerKind::NaturalDraught)
                    .with_fan_speed(AngularVelocity::new::<radian_per_second>(omega))
                    .at_time(Time::new::<second>(10.0))
                    .fan_angle(),
                None,
                "a natural-draught tower has no fan"
            );
        }
    }

    /// Every scatter in the artwork must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so rain and
    /// plume puffs drawn from a real random source would boil between frames.
    /// Evaluate [`tower_hash`] repeatedly at 3 000 (index, salt) sites and
    /// require bitwise-equal results in `[0, 1)`; require the salted draws at
    /// one site to differ; require adjacent indices to decorrelate; and require
    /// [`scatter_point`] to be bitwise stable and to land inside its rectangle.
    ///
    /// **Result (2026-08-12):** 3 000 hash sites re-evaluated three times each,
    /// all bitwise identical and in range; 37 of 40 adjacent index pairs
    /// differed by more than 0.05; the four salted draws at one site were
    /// pairwise distinct; 400 scatter points were stable across repeats and all
    /// lay inside their rectangle. Interpretation: the rain and the plume stand
    /// still between repaints while still looking irregular.
    #[test]
    fn the_scatter_is_deterministic_and_in_range() {
        let mut checked = 0usize;
        for index in -50..250 {
            for salt in 101..111u32 {
                let first = tower_hash(index, 0, salt);
                for _ in 0..3 {
                    assert_eq!(first, tower_hash(index, 0, salt), "hash is not deterministic");
                }
                assert!((0.0..1.0).contains(&first), "hash {first} out of range");
                checked += 1;
            }
        }
        println!("{checked} hash sites re-evaluated");

        let (a, b, c, d) = (
            tower_hash(7, 3, 101),
            tower_hash(7, 3, 102),
            tower_hash(7, 3, 103),
            tower_hash(7, 3, 104),
        );
        assert!((a - b).abs() > 1e-6 && (b - c).abs() > 1e-6 && (c - d).abs() > 1e-6);

        let mut differing = 0;
        for i in 0..40 {
            if (tower_hash(i, 0, 101) - tower_hash(i + 1, 0, 101)).abs() > 0.05 {
                differing += 1;
            }
        }
        println!("{differing}/40 adjacent index pairs decorrelated");
        assert!(
            differing > 30,
            "adjacent sites are too similar ({differing}/40 differ) — the rain will look striped"
        );

        let area = Rect::from_min_size(Pos2::new(12.0, -30.0), Vec2::new(140.0, 55.0));
        for index in 0..200 {
            for salt in [101u32, 113] {
                let first = scatter_point(area, index, salt);
                assert_eq!(first, scatter_point(area, index, salt));
                assert!(
                    area.contains(first),
                    "scatter point {first:?} escaped its area"
                );
            }
        }
    }

    /// Every kind must name itself, its draught mechanism and where it is used,
    /// so a gallery caption can be built without a lookup table elsewhere going
    /// stale.
    #[test]
    fn every_kind_describes_itself() {
        assert_eq!(CoolingTowerKind::ALL.len(), 2);
        for kind in CoolingTowerKind::ALL {
            assert!(!kind.label().is_empty());
            assert!(!kind.description().is_empty());
            assert!(!kind.draught().is_empty());
        }
        assert!(CoolingTowerKind::NaturalDraught
            .draught()
            .contains("buoyancy"));
        assert!(CoolingTowerKind::InducedDraught.draught().contains("fan"));
    }

    /// The display interpolation down the fill must hit its endpoints exactly,
    /// so the deck and the basin read as the two water temperatures the caller
    /// supplied and nothing else.
    #[test]
    fn the_falling_water_gradient_interpolates_between_its_endpoints() {
        let (from, to) = (degc(40.0), degc(30.0));
        assert!(
            (lerp_temperature(from, to, 0.0).get::<kelvin>() - from.get::<kelvin>()).abs() < 1e-9
        );
        assert!(
            (lerp_temperature(from, to, 1.0).get::<kelvin>() - to.get::<kelvin>()).abs() < 1e-9
        );
        assert_eq!(
            lerp_temperature(from, to, -3.0).get::<kelvin>(),
            from.get::<kelvin>()
        );
        assert_eq!(
            lerp_temperature(from, to, 9.0).get::<kelvin>(),
            to.get::<kelvin>()
        );
    }

    /// Corner radii are `u8` in egui, so the helper must saturate rather than
    /// wrap — a wrapped radius would round a basin to nothing.
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
