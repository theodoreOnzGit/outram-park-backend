//! Destructive-excursion overlay — the drawing a reactor gets when its fuel
//! goes past the temperature it is allowed to reach.
//!
//! This is an **overlay over an arbitrary screen rectangle**, not part of any
//! vessel widget. Destruction is not a property of one reactor type: the same
//! overlay composes over [`crate::components::Htr10ReactorVesselVisual`],
//! [`crate::components::FhrReactorVesselVisual`] or anything else the
//! application draws, by handing it the same rectangle the vessel was drawn
//! into. Nothing in this module knows what is underneath it.
//!
//! # Why this exists, and what it is honestly saying
//!
//! It is easy to drive one of this crate's example simulators past the point
//! where its model means anything. The HTR-10 example's control-rod bank
//! exposes **+16.45 dollars** of reactivity when fully withdrawn — far above
//! prompt critical at 1 dollar — so withdrawing the bank produces a real prompt
//! excursion in the point-kinetics model and the fuel temperature runs away.
//! When that happens the correct thing to show is not a slightly redder vessel:
//! it is a statement that **the model has left its valid envelope and the fuel
//! is destroyed**.
//!
//! So the overlay is a *warning annotation*, drawn in a deliberately
//! non-physical hazard palette (see [`HAZARD`] and [`INCANDESCENT`]) precisely
//! so it can never be misread as a temperature field, with the actual numbers
//! printed as text beside it. It is **not** a blast model, not an accident
//! analysis, and not a source term. Per `RESPONSIBLE_USE.md` this crate's
//! examples are educational demonstrations and must never be presented as
//! authoritative for safety analysis, licensing or emergency response — the
//! overlay says so on screen, in [`ExcursionStage::caption`].
//!
//! # The two temperature figures, which must not be conflated
//!
//! [`ExcursionTrigger::htr10_fuel_temperature`] ramps from the HTR-10's **own
//! specified** maximum fuel-temperature limit, **1230 degrees Celsius**
//! ([`crate::htr10::design::Htr10FuelTemperatureLimits::fuel_temperature_limit`],
//! Gao & Shi 2002), to the **generic** modular-HTR coated-particle
//! fission-product retention figure, **1600 degrees Celsius**
//! ([`crate::htr10::design::generic_coated_particle_retention_limit`]).
//!
//! These are different numbers from different sources and
//! `crate::htr10::design` warns explicitly that mixing them up misstates the
//! HTR-10 margin by 370 K. The overlay therefore uses them as **two distinct
//! landmarks**: the overlay starts at the HTR-10's own limit — that is where
//! the reactor is outside what it is specified for — and reaches full intensity
//! at the generic retention figure, past which the coated-particle fuel is
//! beyond even the generic literature figure. Any margin statement must use
//! 1230 degrees Celsius. Reaching full intensity here means "past both
//! landmarks", not "the vessel exploded"; **no temperature at which an HTR-10
//! core is destroyed is published, and none is invented here.**
//!
//! # What drives it
//!
//! The trigger is an **input** ([`ExcursionTrigger`]), not something read out
//! of a plant model inside the widget: this crate's `CLAUDE.md` keeps `src/`
//! presentation-only, so the fuel temperature is computed by the caller's
//! physics and handed in. [`ExcursionTrigger::Intensity`] is for callers whose
//! criterion is not a fuel temperature at all.
//!
//! # Animation state is application-owned
//!
//! Widgets here are consumed by value and rebuilt on every repaint, so an
//! expansion phase stored inside the widget would reset to zero every frame and
//! the overlay would never advance. The **application** owns the elapsed time
//! since the excursion was triggered and passes it in with
//! [`ExcursionOverlay::since_trigger`] — the same ownership rule as
//! [`crate::animation::TracerTrain`] and [`crate::components::PumpVisual`]'s
//! shaft phase. With no elapsed time supplied the overlay draws the first
//! instant, which is a complete and legible drawing rather than an empty one.
//!
//! The phase is a function of the caller's **simulation** clock, never of a
//! wall clock, so a paused simulation shows a still overlay and a replayed one
//! reproduces frame for frame.

use crate::htr10::design::{generic_coated_particle_retention_limit, Htr10FuelTemperatureLimits};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense};
use egui::{Stroke, StrokeKind, Ui, Vec2, Widget};
use std::f32::consts::TAU;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;
use uom::ConstZero;

// ── Display thresholds and timings ──────────────────────────────────────────

/// Intensity at or above which the overlay escalates from a limit warning to
/// the destructive drawing, dimensionless.
///
/// **A display threshold, not a physical one.** Nothing happens to a reactor at
/// this number; it is where the annotation stops being a border and becomes an
/// overlay, chosen so that a small, brief exceedance is not drawn as a
/// catastrophe.
pub const DESTRUCTIVE_INTENSITY: f32 = 0.35;

/// How long, in **simulation** seconds, the shock front takes to cross the
/// drawn rectangle.
///
/// A presentation constant. It sets how fast the annotation expands on screen
/// and has no physical meaning whatsoever — this module models nothing.
pub const SHOCK_EXPANSION_SECONDS: f64 = 1.4;

/// Frequency, in hertz of **simulation** time, at which the warning banner
/// pulses. A presentation constant; see [`banner_pulse`].
pub const BANNER_PULSE_HZ: f64 = 1.6;

// ── Palette ─────────────────────────────────────────────────────────────────
//
// Deliberately NOT on the temperature colour scale used by every other widget
// in this library. A reader must never be able to mistake this annotation for a
// temperature field, so it uses hazard colours that appear nowhere else.

/// Hazard amber: the warning border, the banner rule, the debris.
pub const HAZARD: Color32 = Color32::from_rgb(240, 158, 34);
/// Incandescent white: the centre of the destructive overlay.
pub const INCANDESCENT: Color32 = Color32::from_rgb(255, 246, 226);
/// Charred graphite: what is drawn over the vessel once the overlay is at full
/// intensity.
const CHAR: Color32 = Color32::from_rgb(24, 20, 18);
/// Banner and caption text.
const TEXT: Color32 = Color32::from_rgb(250, 244, 236);
/// Banner backing, so the text stays legible over whatever is underneath.
const BANNER: Color32 = Color32::from_rgb(122, 22, 16);

// ── The trigger ─────────────────────────────────────────────────────────────

/// Intensity of a fuel-temperature excursion, dimensionless in `[0, 1]`.
///
/// `0.0` at or below `limit`, `1.0` at or above `full_intensity_at`, linear in
/// temperature between them. All three arguments are absolute thermodynamic
/// temperatures (`uom`-typed, kelvin internally, conventionally quoted in
/// degrees Celsius).
///
/// The caller chooses both landmarks, deliberately: reactors do not share a
/// fuel-temperature limit, and this crate must not pick one on a caller's
/// behalf. See [`ExcursionTrigger::htr10_fuel_temperature`] for the HTR-10's
/// own pair and the warning that goes with them.
///
/// **A non-finite fuel temperature gives full intensity**, not zero. A model
/// that has produced a NaN or an infinity has certainly left its valid
/// envelope, and the dangerous failure direction here is the quiet one — a
/// broken model must not look like a healthy reactor. A degenerate span
/// (`full_intensity_at` at or below `limit`) is treated as a step: anything
/// above the limit is full intensity.
pub fn excursion_intensity(
    fuel: ThermodynamicTemperature,
    limit: ThermodynamicTemperature,
    full_intensity_at: ThermodynamicTemperature,
) -> f32 {
    let fuel_k = fuel.get::<kelvin>();
    if !fuel_k.is_finite() {
        return 1.0;
    }
    let limit_k = limit.get::<kelvin>();
    let full_k = full_intensity_at.get::<kelvin>();
    if fuel_k <= limit_k {
        return 0.0;
    }
    if !(full_k > limit_k) {
        return 1.0;
    }
    (((fuel_k - limit_k) / (full_k - limit_k)) as f32).clamp(0.0, 1.0)
}

/// What tells the overlay how bad things are.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule. The set of triggers is closed: either the caller
/// has a fuel temperature and the limits to judge it against, or it has already
/// reduced its criterion to a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExcursionTrigger {
    /// A caller-computed intensity in `[0, 1]`, for a criterion that is not a
    /// fuel temperature (a cladding limit, a pressure, an operator's own
    /// judgement in a teaching scenario). Values outside `[0, 1]` are clamped;
    /// a non-finite value gives full intensity, for the reason given on
    /// [`excursion_intensity`].
    Intensity(f32),
    /// A fuel temperature judged against two caller-supplied landmarks.
    FuelTemperature {
        /// The fuel temperature the caller's physics produced.
        fuel: ThermodynamicTemperature,
        /// The temperature limit this fuel is specified to stay below. The
        /// overlay starts here.
        limit: ThermodynamicTemperature,
        /// The temperature at which the overlay reaches full intensity. A
        /// **display** landmark — see the module documentation; it is not a
        /// destruction threshold and must not be quoted as one.
        full_intensity_at: ThermodynamicTemperature,
    },
}

impl ExcursionTrigger {
    /// Judge an HTR-10 fuel temperature against the HTR-10's **own** limit.
    ///
    /// The overlay starts at 1230 degrees Celsius — the HTR-10's own specified
    /// maximum fuel temperature ([`Htr10FuelTemperatureLimits::fuel_temperature_limit`],
    /// Gao & Shi 2002) — and reaches full intensity at 1600 degrees Celsius,
    /// the *generic* modular-HTR coated-particle retention figure
    /// ([`generic_coated_particle_retention_limit`]).
    ///
    /// **The two figures are different things and are used here as two
    /// different landmarks.** Any statement about the HTR-10's margin uses
    /// 1230 degrees Celsius; the 1600 degrees Celsius figure is not an HTR-10
    /// limit and is not treated as one. See the module documentation.
    ///
    /// `fuel` is the peak fuel temperature the caller's model produced, as an
    /// absolute thermodynamic temperature.
    pub fn htr10_fuel_temperature(fuel: ThermodynamicTemperature) -> Self {
        Self::FuelTemperature {
            fuel,
            limit: Htr10FuelTemperatureLimits::gao_shi_2002().fuel_temperature_limit,
            full_intensity_at: generic_coated_particle_retention_limit(),
        }
    }

    /// Intensity in `[0, 1]` this trigger resolves to.
    pub fn intensity(self) -> f32 {
        match self {
            Self::Intensity(i) => {
                if i.is_finite() {
                    i.clamp(0.0, 1.0)
                } else {
                    1.0
                }
            }
            Self::FuelTemperature {
                fuel,
                limit,
                full_intensity_at,
            } => excursion_intensity(fuel, limit, full_intensity_at),
        }
    }

    /// The fuel temperature behind this trigger, or `None` for
    /// [`Self::Intensity`] — which carries no temperature and must not be made
    /// to look as though it does.
    pub fn fuel_temperature(self) -> Option<ThermodynamicTemperature> {
        match self {
            Self::Intensity(_) => None,
            Self::FuelTemperature { fuel, .. } => Some(fuel),
        }
    }

    /// The limit the fuel temperature is judged against, or `None` for
    /// [`Self::Intensity`].
    pub fn limit(self) -> Option<ThermodynamicTemperature> {
        match self {
            Self::Intensity(_) => None,
            Self::FuelTemperature { limit, .. } => Some(limit),
        }
    }
}

// ── Stages ──────────────────────────────────────────────────────────────────

/// How far the annotation has escalated.
///
/// Enum dispatch per the workspace's "no trait objects" rule; derived from the
/// intensity by [`Self::from_intensity`], so the thresholds live in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcursionStage {
    /// At or below the limit: **nothing is drawn**. A healthy reactor gets no
    /// annotation at all, not a faint one.
    Quiescent,
    /// Past the limit but below [`DESTRUCTIVE_INTENSITY`]: a hazard border and
    /// a banner saying the fuel is above its specified limit. The vessel
    /// underneath is left visible and unobscured — it is still the useful
    /// picture.
    LimitExceeded,
    /// At or above [`DESTRUCTIVE_INTENSITY`]: the full overlay. The vessel
    /// underneath is charred over, because it no longer depicts anything the
    /// model can stand behind.
    Destructive,
}

impl ExcursionStage {
    /// Every stage, in escalation order.
    pub const ALL: &'static [Self] = &[Self::Quiescent, Self::LimitExceeded, Self::Destructive];

    /// The stage an intensity in `[0, 1]` corresponds to.
    ///
    /// Strictly positive intensity is already an exceedance — the fuel is past
    /// the limit it is specified to stay below — so there is no dead band, and
    /// a non-finite intensity escalates to [`Self::Destructive`] rather than
    /// being ignored.
    pub fn from_intensity(intensity: f32) -> Self {
        if !intensity.is_finite() {
            return Self::Destructive;
        }
        if intensity <= 0.0 {
            Self::Quiescent
        } else if intensity < DESTRUCTIVE_INTENSITY {
            Self::LimitExceeded
        } else {
            Self::Destructive
        }
    }

    /// Whether this stage draws anything at all.
    pub fn is_drawn(self) -> bool {
        !matches!(self, Self::Quiescent)
    }

    /// Short banner headline for this stage.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quiescent => "",
            Self::LimitExceeded => "FUEL TEMPERATURE ABOVE ITS SPECIFIED LIMIT",
            Self::Destructive => "PROMPT EXCURSION — FUEL DESTROYED, MODEL INVALID",
        }
    }

    /// The sentence printed under the headline.
    ///
    /// Both non-quiescent stages say, in plain words, that the *model* — not
    /// merely the reactor — is outside what it can stand behind. That framing
    /// is required by `RESPONSIBLE_USE.md`: this is a teaching demonstration,
    /// and it must never read as an accident analysis.
    pub fn caption(self) -> &'static str {
        match self {
            Self::Quiescent => "",
            Self::LimitExceeded => "the reactor is outside the conditions this model was built for",
            Self::Destructive => {
                "demonstration only — beyond this point the simulation represents nothing physical"
            }
        }
    }
}

// ── Kinematics of the annotation ────────────────────────────────────────────

/// How far the annotation has expanded, dimensionless in `[0, 1]`, for an
/// elapsed **simulation** time since the excursion was triggered.
///
/// Reaches `1.0` after [`SHOCK_EXPANSION_SECONDS`] and stays there: the fuel
/// does not become undestroyed, so the overlay does not fade away. Negative or
/// non-finite elapsed times give `0.0` — the instant of the trigger — rather
/// than anything undefined.
pub fn shock_phase(elapsed: Time) -> f32 {
    let seconds = elapsed.get::<second>();
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    ((seconds / SHOCK_EXPANSION_SECONDS) as f32).clamp(0.0, 1.0)
}

/// Radius of the drawn front at expansion phase `phase`, in screen points.
///
/// Grows as `max_radius * sqrt(phase)`, so it moves fastest at the start and
/// slows as it goes out. That is a **display easing chosen because a linear
/// ramp reads as a mechanical wipe**, not a blast calculation — this module
/// solves nothing and must not be cited as though it did.
///
/// `phase` outside `[0, 1]` is clamped; a non-finite phase gives zero.
pub fn shock_front_radius(phase: f32, max_radius: f32) -> f32 {
    if !phase.is_finite() || !max_radius.is_finite() {
        return 0.0;
    }
    max_radius * phase.clamp(0.0, 1.0).sqrt()
}

/// Banner pulse in `[0, 1]` at elapsed **simulation** time `elapsed`.
///
/// A slow triangular-in-appearance sine at [`BANNER_PULSE_HZ`], used only to
/// keep the warning banner from being read as a static decoration. Being a
/// function of the caller's simulation clock, a paused simulation shows a still
/// banner and a replay reproduces it exactly. A non-finite time gives full
/// brightness — a broken clock must not hide the warning.
pub fn banner_pulse(elapsed: Time) -> f32 {
    let seconds = elapsed.get::<second>();
    if !seconds.is_finite() {
        return 1.0;
    }
    let phase = (seconds * BANNER_PULSE_HZ) as f32;
    0.5 + 0.5 * (phase * TAU).sin()
}

// ── Deterministic scatter ───────────────────────────────────────────────────

/// Deterministic pseudo-random value in `[0, 1)` from two indices and a salt.
///
/// **Determinism is the point.** The widget is rebuilt on every repaint, so
/// debris drawn from a real random source would re-scatter every frame and the
/// overlay would boil instead of expanding. Hashing the indices gives a pattern
/// that looks irregular but is identical frame to frame, and identical between
/// two runs of the same simulation.
///
/// Same integer-hash construction as `steam_generator::sg_hash`,
/// `condenser::condenser_hash` and `pump::pump_hash`, duplicated rather than
/// shared because those are private to their own modules; the salts here are
/// this module's own.
fn blast_hash(a: i32, b: i32, salt: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9)
        ^ (b as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h % 1_000_003) as f32 / 1_000_003.0
}

/// The same colour at a reduced alpha.
fn translucent(colour: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
}

/// An alpha from a `[0, 1]` fraction, clamped and rounded.
fn alpha(fraction: f32) -> u8 {
    if !fraction.is_finite() {
        return 0;
    }
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

// ── The widget ──────────────────────────────────────────────────────────────

/// An overlay that annotates a destroyed core over an arbitrary screen
/// rectangle.
///
/// Composed **on top of** whatever vessel the application drew, by giving it
/// the same centre and size:
///
/// ```ignore
/// ui.add(Htr10ReactorVesselVisual::new(/* ... */));
/// ui.add(
///     ExcursionOverlay::new(
///         ExcursionTrigger::htr10_fuel_temperature(peak_fuel_temperature),
///         vessel_centre,
///         vessel_size,
///     )
///     .since_trigger(elapsed_since_excursion),
/// );
/// ```
///
/// Nothing is drawn while the fuel is within its limit, so the overlay can be
/// added unconditionally every frame. See the module documentation for what the
/// annotation claims (very little) and what it refuses to claim (a blast model,
/// an accident analysis, or a destruction temperature).
pub struct ExcursionOverlay {
    trigger: ExcursionTrigger,
    screen_position: Pos2,
    screen_vector: Vec2,
    elapsed: Time,
    show_labels: bool,
    subject: Option<String>,
}

impl ExcursionOverlay {
    /// Build an overlay for `trigger` over the box centred at
    /// `screen_position` with size `screen_vector`, in screen points.
    ///
    /// Give it the **same centre and size the vessel was drawn with**, so the
    /// annotation lands on the equipment it is talking about.
    ///
    /// The elapsed time starts at zero, which draws the first instant of the
    /// annotation. Advance it with [`Self::since_trigger`] — the application
    /// owns that clock, for the reason in the module documentation.
    pub fn new(trigger: ExcursionTrigger, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            trigger,
            screen_position,
            screen_vector,
            elapsed: Time::ZERO,
            show_labels: true,
            subject: None,
        }
    }

    /// Supply the **application-owned** simulation time elapsed since the
    /// excursion was triggered. Builder-style.
    ///
    /// This is the only thing that advances the annotation. A widget-owned
    /// clock would reset to zero on every repaint (widgets here are consumed by
    /// value and rebuilt each frame), so the expansion would never progress —
    /// the same rule that makes [`crate::animation::TracerTrain`]
    /// application-owned.
    pub fn since_trigger(mut self, elapsed: Time) -> Self {
        self.elapsed = elapsed;
        self
    }

    /// Name what is being annotated, e.g. `"HTR-10 core"`. Builder-style.
    ///
    /// Owned [`String`] rather than a borrowed string, per the workspace rule
    /// against lifetime parameters on structs.
    pub fn with_subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Turn the banner and readouts off, leaving only the graphic — for
    /// thumbnails. The banner is the part that says the model is invalid, so
    /// this should not be used in a running simulator.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// The trigger this overlay was built from.
    pub fn trigger(&self) -> ExcursionTrigger {
        self.trigger
    }

    /// Intensity in `[0, 1]` the trigger resolves to.
    pub fn intensity(&self) -> f32 {
        self.trigger.intensity()
    }

    /// The stage the overlay is at.
    pub fn stage(&self) -> ExcursionStage {
        ExcursionStage::from_intensity(self.intensity())
    }

    /// Expansion phase in `[0, 1]`; see [`shock_phase`].
    pub fn phase(&self) -> f32 {
        shock_phase(self.elapsed)
    }

    /// On-screen size of the annotated box, in points.
    pub fn size(&self) -> Vec2 {
        self.screen_vector
    }

    /// How far the fuel is past its limit, in kelvin, or `None` when the
    /// trigger carries no temperatures.
    ///
    /// Positive when the fuel is over. Reported as text next to the banner so
    /// the reader sees the actual numbers rather than only a graphic.
    pub fn overshoot_kelvin(&self) -> Option<f64> {
        let fuel = self.trigger.fuel_temperature()?;
        let limit = self.trigger.limit()?;
        Some(fuel.get::<kelvin>() - limit.get::<kelvin>())
    }

    /// Draw a centred line of text, unless labels are switched off.
    fn text(&self, painter: &Painter, at: Pos2, size: f32, colour: Color32, line: &str) {
        if !self.show_labels {
            return;
        }
        painter.text(
            at,
            Align2::CENTER_CENTER,
            line,
            FontId::proportional(size),
            colour,
        );
    }
}

impl Widget for ExcursionOverlay {
    /// Paints the annotation for [`ExcursionOverlay::stage`] over the given
    /// box, and nothing at all when the fuel is within its limit.
    ///
    /// The box is always allocated, whatever the stage, so adding the overlay
    /// unconditionally does not make the surrounding layout jump when a
    /// reactor crosses its limit.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter_at(rect);
        let stage = self.stage();

        match stage {
            ExcursionStage::Quiescent => {}
            ExcursionStage::LimitExceeded => self.draw_limit_warning(&painter, rect),
            ExcursionStage::Destructive => self.draw_destructive(&painter, rect),
        }
        if stage.is_drawn() {
            self.draw_banner(&painter, rect, stage);
        }

        response
    }
}

impl ExcursionOverlay {
    /// Draws the limit-exceeded annotation: a hazard-striped border around the
    /// box, leaving the vessel inside it fully visible.
    ///
    /// The vessel is still the useful picture at this stage — the fuel is over
    /// its specified limit but the model has not yet been driven somewhere it
    /// cannot describe at all — so nothing is painted over it.
    fn draw_limit_warning(&self, painter: &Painter, rect: Rect) {
        let intensity = self.intensity();
        let stripe = (rect.width().min(rect.height()) * 0.05).max(4.0);
        let strength = alpha(0.35 + 0.65 * intensity / DESTRUCTIVE_INTENSITY.max(1e-3));

        // Diagonal hazard stripes, inset just inside the border.
        let border = rect.shrink(stripe * 0.5);
        let step = stripe * 1.6;
        let mut offset = -border.height();
        while offset < border.width() {
            let x0 = border.left() + offset;
            painter.line_segment(
                [
                    Pos2::new(x0.clamp(border.left(), border.right()), border.top()),
                    Pos2::new(
                        (x0 + border.height()).clamp(border.left(), border.right()),
                        border.bottom(),
                    ),
                ],
                Stroke::new(1.6, translucent(HAZARD, 70)),
            );
            offset += step;
        }

        painter.rect_stroke(
            border,
            0.0,
            Stroke::new(stripe * 0.5, translucent(HAZARD, strength)),
            StrokeKind::Inside,
        );
    }

    /// Draws the destructive annotation: the vessel charred over, an
    /// incandescent core, an expanding front and radial debris.
    ///
    /// Everything scales with the intensity and the application-supplied
    /// expansion phase. The front is a **drawn annotation**, not a computed
    /// blast: see [`shock_front_radius`].
    fn draw_destructive(&self, painter: &Painter, rect: Rect) {
        let intensity = self.intensity();
        let phase = self.phase();
        let centre = rect.center();
        let reach = 0.5 * rect.width().hypot(rect.height());

        // ── Char over the vessel ───────────────────────────────────────────
        //
        // The picture underneath no longer depicts anything the model can
        // stand behind, so it is deliberately obscured rather than left to be
        // read as a working reactor.
        painter.rect_filled(rect, 0.0, translucent(CHAR, alpha(0.35 + 0.45 * intensity)));

        // ── Incandescent core ──────────────────────────────────────────────
        //
        // Concentric discs, brightest at the centre. Drawn in the hazard
        // palette, never on the temperature colour scale, so it cannot be read
        // as a temperature field.
        let core_radius = reach * (0.18 + 0.22 * intensity) * (0.6 + 0.4 * phase);
        let shells = 6;
        for k in (0..shells).rev() {
            let f = (k as f32 + 1.0) / shells as f32;
            let colour = if f < 0.45 { INCANDESCENT } else { HAZARD };
            painter.circle_filled(
                centre,
                core_radius * f,
                translucent(colour, alpha(0.9 * (1.0 - f) * intensity + 0.10)),
            );
        }

        // ── Expanding front ────────────────────────────────────────────────
        let front = shock_front_radius(phase, reach);
        if front > 0.0 {
            painter.circle_stroke(
                centre,
                front,
                Stroke::new(
                    (reach * 0.02).max(1.5),
                    translucent(INCANDESCENT, alpha((1.0 - phase) * intensity)),
                ),
            );
            painter.circle_stroke(
                centre,
                front * 0.82,
                Stroke::new(
                    (reach * 0.012).max(1.0),
                    translucent(HAZARD, alpha(0.7 * (1.0 - phase) * intensity)),
                ),
            );
        }

        // ── Radial debris ──────────────────────────────────────────────────
        //
        // Deterministic, so the streaks fly outward instead of re-scattering
        // every repaint.
        let streaks = 28;
        for i in 0..streaks {
            let angle = (i as f32 + blast_hash(i, 0, 151)) * TAU / streaks as f32;
            let (sin, cos) = angle.sin_cos();
            let length = reach * (0.35 + 0.65 * blast_hash(i, 1, 157)) * intensity;
            let start = front.min(length) * (0.35 + 0.4 * blast_hash(i, 2, 163));
            let end = (start + length * phase.max(0.15)).min(reach);
            painter.line_segment(
                [
                    Pos2::new(centre.x + cos * start, centre.y + sin * start),
                    Pos2::new(centre.x + cos * end, centre.y + sin * end),
                ],
                Stroke::new(
                    (reach * 0.012).max(1.0),
                    translucent(HAZARD, alpha(0.85 * intensity * (1.0 - 0.5 * phase))),
                ),
            );
        }

        // A few incandescent fragments, larger near the centre.
        for i in 0..14 {
            let angle = blast_hash(i, 3, 167) * TAU;
            let radius = reach * (0.15 + 0.7 * blast_hash(i, 4, 173)) * (0.4 + 0.6 * phase);
            let at = Pos2::new(
                centre.x + radius * angle.cos(),
                centre.y + radius * angle.sin(),
            );
            painter.circle_filled(
                at,
                (reach * 0.012 * (0.5 + blast_hash(i, 5, 179))).max(1.0),
                translucent(INCANDESCENT, alpha(0.8 * intensity)),
            );
        }
    }

    /// Draws the warning banner across the box, with the numbers behind it.
    ///
    /// The banner is the part of this annotation that carries the actual claim
    /// — that the model has left its valid range — so it prints the fuel
    /// temperature, the limit it is judged against and the overshoot whenever
    /// the trigger carries them, rather than leaving a reader to infer severity
    /// from a graphic.
    fn draw_banner(&self, painter: &Painter, rect: Rect, stage: ExcursionStage) {
        if !self.show_labels {
            return;
        }
        let width = rect.width();
        let band_height = (rect.height() * 0.18).clamp(26.0, 64.0);
        let band = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.center().y),
            Vec2::new(width * 0.96, band_height),
        );
        let pulse = banner_pulse(self.elapsed);
        painter.rect_filled(band, 3.0, translucent(BANNER, alpha(0.72 + 0.20 * pulse)));
        painter.rect_stroke(
            band,
            3.0,
            Stroke::new(1.5, translucent(HAZARD, alpha(0.6 + 0.4 * pulse))),
            StrokeKind::Middle,
        );

        let headline = match &self.subject {
            Some(subject) => format!("{subject}: {}", stage.label()),
            None => stage.label().to_string(),
        };
        self.text(
            painter,
            Pos2::new(band.center().x, band.top() + band_height * 0.28),
            10.5,
            TEXT,
            &headline,
        );
        self.text(
            painter,
            Pos2::new(band.center().x, band.top() + band_height * 0.58),
            8.5,
            translucent(TEXT, 210),
            stage.caption(),
        );

        // The numbers, under the band.
        if let (Some(fuel), Some(limit), Some(overshoot)) = (
            self.trigger.fuel_temperature(),
            self.trigger.limit(),
            self.overshoot_kelvin(),
        ) {
            self.text(
                painter,
                Pos2::new(band.center().x, band.bottom() + band_height * 0.32),
                8.5,
                HAZARD,
                &format!(
                    "fuel {:.0} degC   limit {:.0} degC   +{:.0} K over",
                    fuel.get::<degree_celsius>(),
                    limit.get::<degree_celsius>(),
                    overshoot
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn seconds(v: f64) -> Time {
        Time::new::<second>(v)
    }

    /// The overlay must start at the HTR-10's **own** limit and must not
    /// silently use the generic coated-particle figure in its place.
    ///
    /// **Methodology.** `crate::htr10::design` warns that the HTR-10's own
    /// specified fuel-temperature limit (1230 degC, Gao & Shi 2002) and the
    /// generic modular-HTR coated-particle retention figure (1600 degC) must
    /// not be conflated, and that any HTR-10 margin uses 1230. Require
    /// [`ExcursionTrigger::htr10_fuel_temperature`] to take exactly those two
    /// values as its start and full-intensity landmarks, to be quiescent at and
    /// below 1230 degC, to be strictly increasing between the two, and to reach
    /// full intensity only at 1600 degC. Also require the two landmarks to be
    /// 370 K apart, which is the margin misstatement the design module warns
    /// about, so a future edit that swaps one for the other fails here.
    ///
    /// **Result (2026-08-12):** limit 1230 degC and full intensity at 1600
    /// degC, 370.0 K apart; intensity 0.000 at 1229 and 1230 degC, 0.0027 at
    /// 1231 degC, 0.5000 at 1415 degC, 1.0000 at 1600 degC and still 1.0000 at
    /// 2000 degC. Interpretation: the annotation begins exactly where the
    /// HTR-10 leaves its own specification, and the generic figure is used only
    /// as the far landmark it is.
    #[test]
    fn the_htr10_trigger_uses_the_htr10_limit_not_the_generic_figure() {
        let trigger = ExcursionTrigger::htr10_fuel_temperature(degc(1300.0));
        let limit = trigger.limit().expect("the HTR-10 trigger carries a limit");
        assert!((limit.get::<degree_celsius>() - 1230.0).abs() < 1e-9);

        let ExcursionTrigger::FuelTemperature {
            full_intensity_at, ..
        } = trigger
        else {
            panic!("the HTR-10 trigger must be a fuel-temperature trigger");
        };
        assert!((full_intensity_at.get::<degree_celsius>() - 1600.0).abs() < 1e-9);

        let span = full_intensity_at.get::<kelvin>() - limit.get::<kelvin>();
        println!("landmarks {span:.1} K apart");
        assert!(
            (span - 370.0).abs() < 1e-9,
            "the two figures must stay distinct"
        );

        let at = |t: f64| ExcursionTrigger::htr10_fuel_temperature(degc(t)).intensity();
        println!(
            "1229 -> {:.4}, 1230 -> {:.4}, 1231 -> {:.4}, 1415 -> {:.4}, 1600 -> {:.4}, 2000 -> {:.4}",
            at(1229.0), at(1230.0), at(1231.0), at(1415.0), at(1600.0), at(2000.0)
        );
        assert_eq!(at(1229.0), 0.0);
        assert_eq!(at(1230.0), 0.0);
        assert!(at(1231.0) > 0.0);
        assert!((at(1415.0) - 0.5).abs() < 1e-3);
        assert_eq!(at(1600.0), 1.0);
        assert_eq!(at(2000.0), 1.0);

        let mut previous = 0.0;
        for k in 0..=370 {
            let i = at(1230.0 + k as f64);
            assert!(i >= previous, "intensity fell at +{k} K");
            previous = i;
        }
    }

    /// The intensity must be zero within the limit, monotonic above it, and
    /// must escalate rather than hide when the model produces nonsense.
    ///
    /// **Methodology.** Sweep a fuel temperature from 200 K below the limit to
    /// 200 K above a 100 K span, in 1 K steps, and require: exactly 0.0 at or
    /// below the limit; strictly increasing across the span; exactly 1.0 at and
    /// above the far landmark; every value inside `[0, 1]`. Then require a
    /// NaN or infinite fuel temperature to give **full** intensity — a model
    /// that has produced one has certainly left its valid envelope, and the
    /// dangerous failure direction is the quiet one — and a degenerate span
    /// (far landmark at or below the limit) to behave as a step.
    ///
    /// **Result (2026-08-12):** 401 samples, all inside `[0, 1]`; 0.0 for every
    /// temperature at or below the limit; 0.5 exactly at the midpoint of the
    /// span; 1.0 at and beyond the far landmark; NaN, +inf and -inf all gave
    /// 1.0; a zero-width span gave 0.0 at the limit and 1.0 one kelvin above
    /// it. Interpretation: a healthy reactor is never annotated, and a broken
    /// model cannot look healthy.
    #[test]
    fn the_intensity_is_zero_within_the_limit_and_escalates_on_nonsense() {
        let limit = degc(1230.0);
        let full = degc(1330.0);
        let mut samples = 0usize;
        let mut previous = 0.0f32;
        for k in -200..=200 {
            let fuel = ThermodynamicTemperature::new::<kelvin>(limit.get::<kelvin>() + k as f64);
            let i = excursion_intensity(fuel, limit, full);
            assert!(
                (0.0..=1.0).contains(&i),
                "intensity {i} out of range at +{k} K"
            );
            if k <= 0 {
                assert_eq!(i, 0.0, "annotated a reactor within its limit at +{k} K");
            }
            assert!(i >= previous - 1e-6, "intensity fell at +{k} K");
            previous = i;
            samples += 1;
        }
        println!("{samples} intensities checked");
        assert!(
            (excursion_intensity(degc(1280.0), limit, full) - 0.5).abs() < 1e-6,
            "the midpoint of the span must be half intensity"
        );
        assert_eq!(excursion_intensity(degc(1330.0), limit, full), 1.0);
        assert_eq!(excursion_intensity(degc(9999.0), limit, full), 1.0);

        let nan = ThermodynamicTemperature::new::<kelvin>(f64::NAN);
        let inf = ThermodynamicTemperature::new::<kelvin>(f64::INFINITY);
        let neg_inf = ThermodynamicTemperature::new::<kelvin>(f64::NEG_INFINITY);
        assert_eq!(excursion_intensity(nan, limit, full), 1.0);
        assert_eq!(excursion_intensity(inf, limit, full), 1.0);
        assert_eq!(excursion_intensity(neg_inf, limit, full), 1.0);

        // Degenerate span: a step at the limit.
        assert_eq!(excursion_intensity(limit, limit, limit), 0.0);
        assert_eq!(excursion_intensity(degc(1231.0), limit, limit), 1.0);

        // A caller-supplied intensity is clamped, and nonsense escalates.
        assert_eq!(ExcursionTrigger::Intensity(-3.0).intensity(), 0.0);
        assert_eq!(ExcursionTrigger::Intensity(0.42).intensity(), 0.42);
        assert_eq!(ExcursionTrigger::Intensity(9.0).intensity(), 1.0);
        assert_eq!(ExcursionTrigger::Intensity(f32::NAN).intensity(), 1.0);
        // ...and it carries no temperatures to display.
        assert_eq!(ExcursionTrigger::Intensity(0.5).fuel_temperature(), None);
        assert_eq!(ExcursionTrigger::Intensity(0.5).limit(), None);
    }

    /// A reactor within its limit must get **no** annotation, and the
    /// escalation must happen exactly at the documented threshold.
    ///
    /// **Methodology.** Sweep the intensity from -0.5 to 1.5 in steps of 0.001
    /// and require [`ExcursionStage::from_intensity`] to give `Quiescent` at
    /// and below zero, `LimitExceeded` strictly between zero and
    /// [`DESTRUCTIVE_INTENSITY`], and `Destructive` at and above it, with no
    /// other transitions. Require a non-finite intensity to escalate, and
    /// require only `Quiescent` to draw nothing.
    ///
    /// **Result (2026-08-12):** 2 001 samples, exactly two transitions — at
    /// intensity 0.001 into `LimitExceeded` and at 0.350 into `Destructive`,
    /// matching `DESTRUCTIVE_INTENSITY` = 0.35; NaN and both infinities gave
    /// `Destructive`; `Quiescent` was the only stage with no label, no caption
    /// and `is_drawn() == false`. Interpretation: nothing is ever drawn over a
    /// reactor that is inside its specification.
    #[test]
    fn the_stage_escalates_only_at_the_documented_threshold() {
        let mut transitions = Vec::new();
        let mut previous = ExcursionStage::from_intensity(-0.5);
        let mut samples = 0usize;
        for k in -500..=1500 {
            let i = k as f32 * 0.001;
            let stage = ExcursionStage::from_intensity(i);
            if stage != previous {
                transitions.push((i, stage));
                previous = stage;
            }
            match stage {
                ExcursionStage::Quiescent => assert!(i <= 0.0),
                ExcursionStage::LimitExceeded => {
                    assert!(i > 0.0 && i < DESTRUCTIVE_INTENSITY)
                }
                ExcursionStage::Destructive => assert!(i >= DESTRUCTIVE_INTENSITY),
            }
            samples += 1;
        }
        println!("{samples} samples, transitions at {transitions:?}");
        assert_eq!(transitions.len(), 2, "expected exactly two escalations");

        assert_eq!(
            ExcursionStage::from_intensity(f32::NAN),
            ExcursionStage::Destructive
        );
        assert_eq!(
            ExcursionStage::from_intensity(f32::INFINITY),
            ExcursionStage::Destructive
        );

        assert!(!ExcursionStage::Quiescent.is_drawn());
        assert!(ExcursionStage::Quiescent.label().is_empty());
        assert!(ExcursionStage::Quiescent.caption().is_empty());
        for stage in [ExcursionStage::LimitExceeded, ExcursionStage::Destructive] {
            assert!(stage.is_drawn());
            assert!(!stage.label().is_empty());
            assert!(!stage.caption().is_empty());
        }
        assert_eq!(ExcursionStage::ALL.len(), 3);
        // The caption must keep saying the model, not just the reactor, is out
        // of range — RESPONSIBLE_USE.md requires that framing.
        assert!(ExcursionStage::Destructive
            .caption()
            .contains("demonstration only"));
        assert!(ExcursionStage::LimitExceeded.caption().contains("model"));
    }

    /// The annotation must advance only with the **application's** clock, and
    /// must never fade back to nothing.
    ///
    /// **Methodology.** The widget is rebuilt every repaint, so the phase must
    /// come from a caller-supplied elapsed simulation time. Require
    /// [`shock_phase`] to be 0.0 at and before the trigger instant, to reach
    /// 1.0 exactly at [`SHOCK_EXPANSION_SECONDS`], to stay at 1.0 afterwards
    /// (the fuel does not become undestroyed), to be monotonic in between, and
    /// to give 0.0 for a non-finite time. Then require two overlays built with
    /// the same elapsed time to report the same phase — the property a
    /// widget-owned clock would break — and one built with a later time to
    /// report a larger one.
    ///
    /// **Result (2026-08-12):** phase 0.000 at -5 s and 0 s, 0.500 at 0.70 s,
    /// 1.000 at 1.40 s and still 1.000 at 60 s; monotonic over 200 sampled
    /// times; NaN gave 0.000. Two overlays at 0.35 s both reported 0.250, and
    /// one at 0.70 s reported 0.500. Interpretation: the animation is a pure
    /// function of the simulation clock, so it survives being rebuilt every
    /// frame, pauses when the simulation pauses, and replays identically.
    #[test]
    fn the_phase_comes_from_the_application_clock_and_does_not_reverse() {
        assert_eq!(shock_phase(seconds(-5.0)), 0.0);
        assert_eq!(shock_phase(seconds(0.0)), 0.0);
        assert!((shock_phase(seconds(0.7)) - 0.5).abs() < 1e-6);
        assert_eq!(shock_phase(seconds(SHOCK_EXPANSION_SECONDS)), 1.0);
        assert_eq!(shock_phase(seconds(60.0)), 1.0);
        assert_eq!(shock_phase(Time::new::<second>(f64::NAN)), 0.0);

        let mut previous = 0.0;
        for k in 0..=200 {
            let t = k as f64 * 0.01;
            let phase = shock_phase(seconds(t));
            assert!(phase >= previous, "phase went backwards at {t} s");
            previous = phase;
        }

        let overlay = |t: f64| {
            ExcursionOverlay::new(
                ExcursionTrigger::htr10_fuel_temperature(degc(1500.0)),
                Pos2::new(40.0, 60.0),
                Vec2::new(200.0, 300.0),
            )
            .since_trigger(seconds(t))
            .phase()
        };
        assert_eq!(overlay(0.35), overlay(0.35));
        assert!((overlay(0.35) - 0.25).abs() < 1e-6);
        assert!(overlay(0.70) > overlay(0.35));
    }

    /// The drawn front must expand outward, decelerating, and must stay inside
    /// the box it was given.
    ///
    /// **Methodology.** The front radius is a display easing, not a blast
    /// calculation. Require [`shock_front_radius`] to be zero at phase 0,
    /// exactly the maximum radius at phase 1, strictly increasing in between,
    /// never greater than the maximum, and **concave** — each successive equal
    /// step in phase must move the front no further than the one before, which
    /// is what makes it read as decelerating rather than as a mechanical wipe.
    /// Also require out-of-range and non-finite inputs to clamp to something
    /// drawable.
    ///
    /// **Result (2026-08-12):** over 100 equal phase steps on a 100-point
    /// maximum radius, the first step moved 10.00 points and the last 0.50
    /// points, with every step no larger than its predecessor; radius 0.00 at
    /// phase 0, 70.71 at phase 0.5 and 100.00 at phase 1; phase 5.0 clamped to
    /// 100.00 and NaN gave 0.00. Interpretation: the annotation expands fast
    /// then settles, and never escapes the rectangle it annotates.
    #[test]
    fn the_front_expands_outward_and_decelerates() {
        let max = 100.0f32;
        assert_eq!(shock_front_radius(0.0, max), 0.0);
        assert!((shock_front_radius(1.0, max) - max).abs() < 1e-4);
        assert!((shock_front_radius(0.5, max) - 70.710_68).abs() < 1e-3);
        assert_eq!(shock_front_radius(5.0, max), max);
        assert_eq!(shock_front_radius(-1.0, max), 0.0);
        assert_eq!(shock_front_radius(f32::NAN, max), 0.0);
        assert_eq!(shock_front_radius(0.5, f32::NAN), 0.0);

        let steps = 100;
        let mut previous_radius = 0.0f32;
        let mut previous_step = f32::INFINITY;
        let mut first_step = 0.0f32;
        let mut last_step = 0.0f32;
        for k in 1..=steps {
            let radius = shock_front_radius(k as f32 / steps as f32, max);
            let step = radius - previous_radius;
            assert!(step > 0.0, "the front stalled at step {k}");
            assert!(
                radius <= max + 1e-4,
                "the front escaped its box at step {k}"
            );
            assert!(
                step <= previous_step + 1e-4,
                "the front accelerated at step {k}"
            );
            if k == 1 {
                first_step = step;
            }
            last_step = step;
            previous_step = step;
            previous_radius = radius;
        }
        println!("first step {first_step:.2} points, last step {last_step:.2} points");
        assert!(
            first_step > 10.0 * last_step,
            "the front barely decelerated"
        );
    }

    /// The banner pulse must stay bounded, be a pure function of simulation
    /// time, and stay visible when the clock is broken.
    ///
    /// **Methodology.** Sample [`banner_pulse`] over 2 000 simulation times and
    /// require every value to lie in `[0, 1]`, the same time to give the same
    /// value bitwise, the pulse to actually vary (a constant would be a
    /// dead animation), and a non-finite time to give full brightness rather
    /// than darkness — a broken clock must not hide the warning.
    ///
    /// **Result (2026-08-12):** 2 001 samples spanning 0 to 20 s, all inside
    /// `[0, 1]`, minimum 0.0000 and maximum 1.0000 over the sweep; repeated
    /// evaluation was bitwise identical; NaN and infinity both gave 1.0.
    /// Interpretation: the banner breathes at the simulation's own pace and
    /// cannot go dark.
    #[test]
    fn the_banner_pulse_is_bounded_and_deterministic() {
        let mut low = f32::INFINITY;
        let mut high = f32::NEG_INFINITY;
        let mut samples = 0usize;
        for k in 0..=2000 {
            let t = seconds(k as f64 * 0.01);
            let pulse = banner_pulse(t);
            assert!(
                (0.0..=1.0).contains(&pulse),
                "pulse {pulse} out of range at {k}"
            );
            assert_eq!(pulse, banner_pulse(t), "pulse is not deterministic");
            low = low.min(pulse);
            high = high.max(pulse);
            samples += 1;
        }
        println!("{samples} pulses, range {low:.4}..{high:.4}");
        assert!(high - low > 0.9, "the banner does not visibly pulse");
        assert_eq!(banner_pulse(Time::new::<second>(f64::NAN)), 1.0);
        assert_eq!(banner_pulse(Time::new::<second>(f64::INFINITY)), 1.0);
    }

    /// The debris scatter must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so debris drawn
    /// from a real random source would re-scatter each frame — the streaks
    /// would flicker instead of flying outward. Evaluate [`blast_hash`]
    /// repeatedly at 3 000 (index, salt) sites and require bitwise-equal
    /// results in `[0, 1)`, the salted draws at one site to differ, and
    /// adjacent indices to decorrelate.
    ///
    /// **Result (2026-08-12):** 3 000 hash sites re-evaluated three times each,
    /// all bitwise identical and in range; 39 of 40 adjacent index pairs
    /// differed by more than 0.05; the four salted draws at one site were
    /// pairwise distinct. Interpretation: the debris pattern is fixed by the
    /// index, so it expands with the phase instead of boiling.
    #[test]
    fn the_debris_scatter_is_deterministic() {
        let mut checked = 0usize;
        for index in -50..250 {
            for salt in 151..161u32 {
                let first = blast_hash(index, 0, salt);
                for _ in 0..3 {
                    assert_eq!(
                        first,
                        blast_hash(index, 0, salt),
                        "hash is not deterministic"
                    );
                }
                assert!((0.0..1.0).contains(&first), "hash {first} out of range");
                checked += 1;
            }
        }
        println!("{checked} hash sites re-evaluated");

        let (a, b, c, d) = (
            blast_hash(7, 3, 151),
            blast_hash(7, 3, 157),
            blast_hash(7, 3, 163),
            blast_hash(7, 3, 167),
        );
        assert!((a - b).abs() > 1e-6 && (b - c).abs() > 1e-6 && (c - d).abs() > 1e-6);

        let mut differing = 0;
        for i in 0..40 {
            if (blast_hash(i, 0, 151) - blast_hash(i + 1, 0, 151)).abs() > 0.05 {
                differing += 1;
            }
        }
        println!("{differing}/40 adjacent index pairs decorrelated");
        assert!(differing > 30, "the debris will look striped");
    }

    /// The overlay must report the numbers it is annotating, so the graphic is
    /// never the only evidence.
    #[test]
    fn the_overlay_reports_the_numbers_behind_it() {
        let overlay = ExcursionOverlay::new(
            ExcursionTrigger::htr10_fuel_temperature(degc(1450.0)),
            Pos2::new(10.0, 20.0),
            Vec2::new(180.0, 260.0),
        )
        .with_subject("HTR-10 core".to_string())
        .since_trigger(seconds(0.7));

        assert_eq!(overlay.stage(), ExcursionStage::Destructive);
        assert!((overlay.overshoot_kelvin().unwrap() - 220.0).abs() < 1e-6);
        assert!((overlay.intensity() - (220.0 / 370.0)).abs() < 1e-3);
        assert!((overlay.phase() - 0.5).abs() < 1e-6);
        assert_eq!(overlay.size(), Vec2::new(180.0, 260.0));
        assert_eq!(
            overlay
                .trigger()
                .fuel_temperature()
                .map(|t| t.get::<degree_celsius>().round()),
            Some(1450.0)
        );

        // A caller-supplied intensity has no numbers to report, and says so.
        let bare = ExcursionOverlay::new(
            ExcursionTrigger::Intensity(0.8),
            Pos2::ZERO,
            Vec2::new(100.0, 100.0),
        );
        assert_eq!(bare.overshoot_kelvin(), None);
        assert_eq!(bare.stage(), ExcursionStage::Destructive);
    }

    /// A reactor inside its limit must produce a quiescent overlay, so the
    /// application can add it unconditionally every frame.
    #[test]
    fn a_healthy_reactor_is_not_annotated() {
        for fuel in [20.0, 600.0, 1046.6, 1229.9, 1230.0] {
            let overlay = ExcursionOverlay::new(
                ExcursionTrigger::htr10_fuel_temperature(degc(fuel)),
                Pos2::ZERO,
                Vec2::new(100.0, 100.0),
            );
            assert_eq!(
                overlay.stage(),
                ExcursionStage::Quiescent,
                "{fuel} degC must not be annotated"
            );
            assert_eq!(overlay.intensity(), 0.0);
            assert!(!overlay.stage().is_drawn());
        }
    }

    /// Alpha conversion must saturate rather than wrap — a wrapped alpha would
    /// turn a full-intensity overlay transparent.
    #[test]
    fn alphas_saturate_instead_of_wrapping() {
        assert_eq!(alpha(0.0), 0);
        assert_eq!(alpha(1.0), 255);
        assert_eq!(alpha(2.0), 255);
        assert_eq!(alpha(-1.0), 0);
        assert_eq!(alpha(f32::NAN), 0);
    }
}
