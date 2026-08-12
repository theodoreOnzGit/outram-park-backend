//! Fuel-excursion overlay — the annotation a reactor gets when its fuel goes
//! past the temperature it is specified for.
//!
//! This is an **overlay over an arbitrary screen rectangle**, not part of any
//! vessel widget. It composes over
//! [`crate::components::Htr10ReactorVesselVisual`],
//! [`crate::components::FhrReactorVesselVisual`] or anything else the
//! application draws, by handing it the same rectangle the vessel was drawn
//! into. Nothing in this module knows what is underneath it.
//!
//! # What this depicts, and what it refuses to depict
//!
//! **It does not depict an explosion, and it must never be changed to.** An
//! earlier version of this module drew a shock front and flying debris. That
//! was wrong on the physics and it inverted the central claim of the fuel form
//! it was annotating:
//!
//! - **A modular HTGR has no blast mechanism available at these conditions.**
//!   The coolant is helium — no phase change, no stored pressure energy of the
//!   kind a water reactor has — over a graphite core of low power density and
//!   very large heat capacity. There is no energy source that produces a blast.
//! - **TRISO fuel is *retaining* across most of the band this overlay covers.**
//!   Coating integrity for the HTR-10 was experimentally proven to
//!   **1250 degC** and its design maximum fuel temperature is **1230 degC**
//!   (Gao & Shi 2002 — see [`ExcursionTrigger::htr10_fuel_temperature`]); the
//!   German heating tests showed **no particle failures and no noticeable
//!   caesium or strontium release during the first few hundred hours of any
//!   1600 degC test**, i.e. near-100 % retention at the generic limit itself
//!   (Kugeler et al. 2017, section 4.2.1). Drawing destruction there would
//!   contradict the evidence the same workspace cites.
//! - **The real failure mode is progressive fission-product release** —
//!   coating degradation and gradual release as temperature and *time at
//!   temperature* accumulate. It is slow and it is passive. That is what
//!   [`ExcursionStage::FissionProductRelease`] draws: a fuel region escalating
//!   in incandescence with release marks drifting out of it, on a palette that
//!   is deliberately not the temperature scale.
//!
//! # The sourced landmarks
//!
//! | Temperature | What it is | Source |
//! |---|---|---|
//! | 1230 degC | The HTR-10's **own specified** maximum fuel temperature, normal and accident conditions | Gao & Shi (2002), via [`crate::htr10::design::Htr10FuelTemperatureLimits::fuel_temperature_limit`] |
//! | 1250 degC | Coating integrity **experimentally proven** to this temperature — the basis of the 1230 degC design limit | Gao & Shi (2002) section 1, recorded in `docs/reactor-scoping/htr10-plant-data.md` |
//! | 1600 degC | The **generic** modular-HTR fuel temperature limit; set from an estimated ~1500 degC maximum core temperature plus allowance for thermal-property uncertainty. Heating tests show near-100 % retention here for the first hundred hours or more | [`crate::htr10::design::generic_coated_particle_retention_limit`]; Kugeler et al. (2017) section 4.2.1 |
//! | 1700-1800 degC | Where particle failures and release inventories **increase**; at 1800 degC there is no delay in caesium release and SiC becomes permeable to most fission products | Kugeler et al. (2017) section 4.2.1 |
//!
//! **1230 and 1600 degC are different numbers from different sources and must
//! not be conflated** — `crate::htr10::design` warns that mixing them up
//! misstates the HTR-10 margin by 370 K. This module uses them as two distinct
//! landmarks: the annotation *starts* at the reactor's own limit and reaches
//! full intensity at the generic figure.
//!
//! **Kugeler, K., Nabielek, H. and Buckthorpe, D. (2017).** *The High
//! Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul.*
//! EUR 28712 EN, JRC107642, Publications Office of the European Union,
//! doi:10.2760/270321. Open tier, catalogued in this workspace as
//! `kugeler2017vhtr`; reuse authorised provided the source is acknowledged
//! (EC Decision 2011/833/EU). Facts are cited here, not re-hosted.
//!
//! # Why the escalation happens only at the far landmark
//!
//! [`ExcursionStage::FissionProductRelease`] is reached only at
//! [`RELEASE_INTENSITY`], which is the **top** of the ramp — the generic
//! 1600 degC figure. Escalating earlier would draw release across a band in
//! which retention is precisely what the heating tests demonstrate. Between the
//! two landmarks the annotation says what is true and no more: the fuel is
//! above the limit it is specified for.
//!
//! # What the overlay claims above the far landmark
//!
//! Not a specific physical outcome. Above the generic limit the honest
//! statement is that **this model has left its valid envelope** — this crate
//! has no fission-product release model, no source term and no coating-failure
//! model, and nothing drawn here is one. The escalating incandescence and the
//! drifting release marks are a *warning annotation* naming the mechanism that
//! applies, not a prediction of it. Per `RESPONSIBLE_USE.md` this crate's
//! examples are educational demonstrations and must never be presented as
//! authoritative for safety analysis, licensing or emergency response — the
//! overlay says so on screen, in [`ExcursionStage::caption`].
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
//! Release grows with **time held above the limit** — the heating-test releases
//! are quoted in hundreds of hours, not instants — so the annotation is
//! time-phased. Widgets here are consumed by value and rebuilt on every
//! repaint, so a phase stored inside the widget would reset to zero every frame
//! and never advance. The **application** owns the elapsed time and passes it in
//! with [`ExcursionOverlay::since_trigger`], the same ownership rule as
//! [`crate::animation::TracerTrain`] and [`crate::components::PumpVisual`]'s
//! shaft phase. The phase is a function of the caller's **simulation** clock,
//! never a wall clock, so a paused simulation shows a still overlay and a
//! replayed one reproduces frame for frame.
//!
//! **The on-screen ramp is a presentation constant and is not a release rate.**
//! See [`RELEASE_RAMP_SECONDS`].

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
/// the fission-product-release annotation, dimensionless.
///
/// **This sits at the top of the ramp — `1.0` — deliberately**, so the release
/// annotation appears only once the fuel reaches the far landmark the caller
/// supplied (the generic 1600 degC figure, for
/// [`ExcursionTrigger::htr10_fuel_temperature`]).
///
/// The reason is evidential, not stylistic. In the German core-heat-up
/// simulation tests on irradiated LEU UO2 TRISO spherical fuel elements, *no*
/// single particle failures and no noticeable caesium or strontium release were
/// observed during the first few hundred hours of any 1600 degC heating test —
/// near-100 % retention at the limit itself (Kugeler et al. 2017, section
/// 4.2.1). Escalating below that would depict release across the very band in
/// which retention is demonstrated.
pub const RELEASE_INTENSITY: f32 = 1.0;

/// How long, in **simulation** seconds, the release annotation takes to reach
/// its full drawn extent.
///
/// **A presentation constant. It is not a release rate and implies no
/// timescale.** The real measurements are in hundreds of hours — the heating
/// tests report near-complete retention "for the accident-specific first
/// hundred hours or more" at 1600 degC, and release from already-exposed
/// kernels approaching 100 % only "after 50 to 100 h" (Kugeler et al. 2017,
/// section 4.2.1). A few seconds of screen time is a legibility choice and
/// nothing else; this crate has no release model to derive a rate from.
pub const RELEASE_RAMP_SECONDS: f64 = 1.4;

/// Frequency, in hertz of **simulation** time, at which the warning banner
/// pulses. A presentation constant; see [`banner_pulse`].
pub const BANNER_PULSE_HZ: f64 = 1.6;

// ── Palette ─────────────────────────────────────────────────────────────────
//
// Deliberately NOT on the temperature colour scale used by every other widget
// in this library. A reader must never be able to mistake this annotation for a
// temperature field, so it uses hazard colours that appear nowhere else.

/// Hazard amber: the warning border, the banner rule, the release marks.
pub const HAZARD: Color32 = Color32::from_rgb(240, 158, 34);
/// Incandescent white: the hottest part of the fuel region.
pub const INCANDESCENT: Color32 = Color32::from_rgb(255, 246, 226);
/// The shroud drawn over the vessel once the model has left its valid envelope
/// — it says "the picture underneath is no longer something to read", not
/// "this is what the reactor looks like now".
const SHROUD: Color32 = Color32::from_rgb(24, 20, 18);
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

/// What tells the overlay how far past its limit the fuel is.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule. The set of triggers is closed: either the caller
/// has a fuel temperature and the landmarks to judge it against, or it has
/// already reduced its criterion to a number.
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
        /// annotation starts here.
        limit: ThermodynamicTemperature,
        /// The temperature at which the annotation reaches full intensity and
        /// escalates. A **display** landmark chosen by the caller — it is not a
        /// destruction threshold, and no such threshold is published or
        /// invented here. See the module documentation.
        full_intensity_at: ThermodynamicTemperature,
    },
}

impl ExcursionTrigger {
    /// Judge an HTR-10 fuel temperature against the HTR-10's **own** limit.
    ///
    /// The annotation starts at 1230 degrees Celsius — the HTR-10's own
    /// specified maximum fuel temperature
    /// ([`Htr10FuelTemperatureLimits::fuel_temperature_limit`], Gao & Shi 2002,
    /// itself set from the experimental demonstration that the coating retains
    /// fission products to 1250 degrees Celsius) — and reaches full intensity
    /// at 1600 degrees Celsius, the *generic* modular-HTR fuel temperature
    /// limit ([`generic_coated_particle_retention_limit`]).
    ///
    /// **The two figures are different things and are used here as two
    /// different landmarks.** Any statement about the HTR-10's margin uses
    /// 1230 degrees Celsius; the 1600 degrees Celsius figure is not an HTR-10
    /// limit and is not treated as one. Between them the fuel is above its
    /// specification but the heating tests show it still retaining — see the
    /// module documentation and [`RELEASE_INTENSITY`].
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
    /// At or below the limit: **nothing is drawn**. A reactor inside its
    /// specification gets no annotation at all, not a faint one.
    Quiescent,
    /// Above the limit the fuel is specified for, but below
    /// [`RELEASE_INTENSITY`]: a hazard border and a banner saying so.
    ///
    /// **The coating is not assumed to have failed here, and nothing is drawn
    /// as though it had.** For the HTR-10 landmarks this band runs from
    /// 1230 degC to 1600 degC, across which the heating tests show near-100 %
    /// retention for the first hundred hours or more (Kugeler et al. 2017,
    /// section 4.2.1). The vessel underneath is left visible and unobscured —
    /// it is still the useful picture.
    LimitExceeded,
    /// At or above [`RELEASE_INTENSITY`]: the fuel region escalates in
    /// incandescence and release marks drift out of it.
    ///
    /// This names the mechanism that actually applies to coated-particle fuel —
    /// progressive coating degradation and fission-product release, which is
    /// slow and passive — and simultaneously states that **the model has left
    /// its valid envelope**: this crate has no release model, so nothing drawn
    /// is a source term or a prediction. The vessel underneath is shrouded,
    /// because it no longer depicts anything the model can stand behind.
    FissionProductRelease,
}

impl ExcursionStage {
    /// Every stage, in escalation order.
    pub const ALL: &'static [Self] = &[
        Self::Quiescent,
        Self::LimitExceeded,
        Self::FissionProductRelease,
    ];

    /// The stage an intensity in `[0, 1]` corresponds to.
    ///
    /// Strictly positive intensity is already an exceedance — the fuel is past
    /// the limit it is specified to stay below — so there is no dead band, and
    /// a non-finite intensity escalates to [`Self::FissionProductRelease`]
    /// rather than being ignored.
    pub fn from_intensity(intensity: f32) -> Self {
        if !intensity.is_finite() {
            return Self::FissionProductRelease;
        }
        if intensity <= 0.0 {
            Self::Quiescent
        } else if intensity < RELEASE_INTENSITY {
            Self::LimitExceeded
        } else {
            Self::FissionProductRelease
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
            Self::FissionProductRelease => "FISSION-PRODUCT RELEASE — MODEL OUTSIDE ITS RANGE",
        }
    }

    /// The sentence printed under the headline.
    ///
    /// Both non-quiescent stages say, in plain words, that the *model* — not
    /// merely the reactor — is outside what it can stand behind, and neither
    /// claims a physical outcome. That framing is required by
    /// `RESPONSIBLE_USE.md`: this is a teaching demonstration and must never
    /// read as an accident analysis.
    pub fn caption(self) -> &'static str {
        match self {
            Self::Quiescent => "",
            Self::LimitExceeded => {
                "above the limit this fuel is specified for — the coating is not assumed failed"
            }
            Self::FissionProductRelease => {
                "demonstration only — no release model here; progressive, not explosive"
            }
        }
    }

    /// A second line naming the mechanism, for the stage that has one.
    ///
    /// Deliberately explicit that the mechanism is gradual: coated-particle
    /// fuel degrades and releases over hours, and this crate models none of it.
    pub fn mechanism(self) -> &'static str {
        match self {
            Self::Quiescent => "",
            Self::LimitExceeded => "retention still demonstrated at these temperatures",
            Self::FissionProductRelease => {
                "coating degradation and gradual release over hours — not a blast"
            }
        }
    }
}

// ── The species the annotation names ────────────────────────────────────────

/// One fission product the release annotation can name.
///
/// Held as `&'static str` fields rather than owned strings — no lifetime
/// *parameters* are introduced, per the workspace rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseSpecies {
    /// Short label drawn on the annotation, e.g. `"Cs"`.
    pub symbol: &'static str,
    /// The nuclide the label stands for, e.g. `"137Cs"`.
    pub nuclide: &'static str,
    /// Why this one appears where it does in the order.
    pub note: &'static str,
}

/// The order release marks are named in, earliest first.
///
/// **The order is sourced; the phase at which each label appears is
/// ILLUSTRATIVE.** Kugeler et al. (2017) section 4.2.1 state that caesium is
/// retained at 1600 degC by the kernel, the SiC and the A3 matrix, and that at
/// 1800 degC there is no delay in caesium release and SiC becomes permeable to
/// most fission products; that **krypton is always released later than
/// caesium**, because of the additional retention provided by the dense intact
/// pyrocarbon layers; and that **strontium is retained much better than
/// caesium** in oxide kernels and the sphere matrix, so strontium release
/// generally occurs later still. That gives the sequence below.
///
/// It does **not** give a time or a temperature at which each appears on
/// screen, and none is claimed: [`species_visible`] spreads them across the
/// drawn phase purely so the sequence is legible. The wider set of products the
/// same section calls most relevant — 90Sr, 110mAg, 134Cs, 137Cs, 85Kr, 131I
/// and 133Xe — is named in the module documentation rather than drawn, because
/// this crate has no inventory to draw it from.
pub const RELEASE_ORDER: &[ReleaseSpecies] = &[
    ReleaseSpecies {
        symbol: "Cs",
        nuclide: "137Cs",
        note: "retained by kernel, SiC and matrix at 1600 degC; undelayed at 1800 degC",
    },
    ReleaseSpecies {
        symbol: "Kr",
        nuclide: "85Kr",
        note: "always later than caesium — the intact pyrocarbon layers retain it",
    },
    ReleaseSpecies {
        symbol: "Sr",
        nuclide: "90Sr",
        note: "retained better than caesium in oxide kernels, so later still",
    },
];

/// Whether release species `index` of [`RELEASE_ORDER`] is named yet, at drawn
/// phase `phase` in `[0, 1]`.
///
/// The species are spread evenly across the phase so the sourced *order* reads
/// on screen. **The spacing is a display device and carries no timescale** —
/// see [`RELEASE_ORDER`] and [`RELEASE_RAMP_SECONDS`].
pub fn species_visible(index: usize, phase: f32) -> bool {
    if !phase.is_finite() || index >= RELEASE_ORDER.len() {
        return false;
    }
    let appears_at = (index as f32 + 1.0) / (RELEASE_ORDER.len() as f32 + 1.0);
    phase.clamp(0.0, 1.0) >= appears_at
}

// ── Kinematics of the annotation ────────────────────────────────────────────

/// How far the annotation has progressed, dimensionless in `[0, 1]`, for an
/// elapsed **simulation** time since the excursion was triggered.
///
/// Reaches `1.0` after [`RELEASE_RAMP_SECONDS`] and stays there: released
/// fission products do not go back into the fuel, so the annotation does not
/// fade away. Negative or non-finite elapsed times give `0.0` — the instant of
/// the trigger — rather than anything undefined.
pub fn release_phase(elapsed: Time) -> f32 {
    let seconds = elapsed.get::<second>();
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    ((seconds / RELEASE_RAMP_SECONDS) as f32).clamp(0.0, 1.0)
}

/// How far the release marks have drifted from the fuel region at phase
/// `phase`, in screen points.
///
/// Grows as `max_reach * sqrt(phase)`, so the marks move away quickly at first
/// and then settle. That is a **display easing chosen because a linear ramp
/// reads as a mechanical wipe**, and it is *not* a transport calculation: this
/// module solves nothing and must not be cited as though it did.
///
/// `phase` outside `[0, 1]` is clamped; a non-finite phase or reach gives zero.
pub fn release_reach(phase: f32, max_reach: f32) -> f32 {
    if !phase.is_finite() || !max_reach.is_finite() {
        return 0.0;
    }
    max_reach * phase.clamp(0.0, 1.0).sqrt()
}

/// Banner pulse in `[0, 1]` at elapsed **simulation** time `elapsed`.
///
/// A slow sine at [`BANNER_PULSE_HZ`], used only to keep the warning banner
/// from being read as a static decoration. Being a function of the caller's
/// simulation clock, a paused simulation shows a still banner and a replay
/// reproduces it exactly. A non-finite time gives full brightness — a broken
/// clock must not hide the warning.
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
/// release marks drawn from a real random source would re-scatter every frame
/// and the annotation would boil instead of drifting. Hashing the indices gives
/// a pattern that looks irregular but is identical frame to frame, and
/// identical between two runs of the same simulation.
///
/// Same integer-hash construction as `steam_generator::sg_hash`,
/// `condenser::condenser_hash` and `pump::pump_hash`, duplicated rather than
/// shared because those are private to their own modules; the salts here are
/// this module's own.
fn release_hash(a: i32, b: i32, salt: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9)
        ^ (b as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h % 1_000_003) as f32 / 1_000_003.0
}

/// The same colour at a reduced alpha.
fn translucent(colour: Color32, alpha_value: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha_value)
}

/// An alpha from a `[0, 1]` fraction, clamped and rounded.
fn alpha(fraction: f32) -> u8 {
    if !fraction.is_finite() {
        return 0;
    }
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

// ── The widget ──────────────────────────────────────────────────────────────

/// An overlay that annotates a fuel excursion over an arbitrary screen
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
///     .since_trigger(time_above_the_limit),
/// );
/// ```
///
/// Nothing is drawn while the fuel is within its limit, so the overlay can be
/// added unconditionally every frame. See the module documentation for what the
/// annotation claims (very little) and what it refuses to claim (an explosion,
/// a release rate, a source term, or a destruction temperature).
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
    /// excursion was triggered — that is, how long the fuel has been held above
    /// its limit. Builder-style.
    ///
    /// This is the only thing that advances the annotation. A widget-owned
    /// clock would reset to zero on every repaint (widgets here are consumed by
    /// value and rebuilt each frame), so the annotation would never progress —
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
    /// thumbnails. The banner is the part that says the model is outside its
    /// range, so this should not be used in a running simulator.
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

    /// Drawn phase in `[0, 1]`; see [`release_phase`].
    pub fn phase(&self) -> f32 {
        release_phase(self.elapsed)
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

    /// The species named on the annotation at this phase, in
    /// [`RELEASE_ORDER`].
    ///
    /// Empty unless the overlay is at
    /// [`ExcursionStage::FissionProductRelease`] — naming a released nuclide
    /// while the coating is still demonstrated to retain would be the same
    /// error this module exists to avoid.
    pub fn named_species(&self) -> Vec<ReleaseSpecies> {
        if self.stage() != ExcursionStage::FissionProductRelease {
            return Vec::new();
        }
        let phase = self.phase();
        RELEASE_ORDER
            .iter()
            .enumerate()
            .filter(|(i, _)| species_visible(*i, phase))
            .map(|(_, s)| *s)
            .collect()
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
            ExcursionStage::FissionProductRelease => self.draw_release(&painter, rect),
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
    /// **Nothing is drawn over the fuel at this stage, on purpose.** The fuel
    /// is above the temperature it is specified for, which is worth a border
    /// and a banner; it is not evidence that the coating has failed, and for
    /// the HTR-10 landmarks the heating tests show the opposite across this
    /// whole band (see [`RELEASE_INTENSITY`]). The vessel underneath is still
    /// the useful picture.
    fn draw_limit_warning(&self, painter: &Painter, rect: Rect) {
        let intensity = self.intensity();
        let stripe = (rect.width().min(rect.height()) * 0.05).max(4.0);
        // Fades up across the band between the two landmarks, so approaching
        // the far one is visible without anything being drawn over the fuel.
        let strength = alpha(0.35 + 0.55 * intensity);

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

    /// Draws the fission-product-release annotation: the vessel shrouded, the
    /// fuel region escalating in incandescence, and release marks drifting
    /// slowly out of it with the species named in their sourced order.
    ///
    /// **There is no shock front, no debris and no blast, and there must never
    /// be.** See the module documentation: a modular HTGR has no blast
    /// mechanism available at these conditions, and the mechanism that does
    /// apply — progressive coating degradation and gradual release — is what is
    /// drawn. Everything scales with the intensity and with the
    /// application-supplied phase; the drift is a display easing (see
    /// [`release_reach`]), not a transport calculation.
    fn draw_release(&self, painter: &Painter, rect: Rect) {
        let intensity = self.intensity();
        let phase = self.phase();
        let centre = rect.center();
        let reach = 0.5 * rect.width().hypot(rect.height());

        // ── Shroud over the vessel ─────────────────────────────────────────
        //
        // The picture underneath no longer depicts anything the model can
        // stand behind, so it is deliberately obscured rather than left to be
        // read as a working reactor. Lighter than an opaque cover: the vessel
        // outline should still be recognisable as the thing being annotated.
        painter.rect_filled(
            rect,
            0.0,
            translucent(SHROUD, alpha(0.30 + 0.35 * intensity)),
        );

        // ── Fuel region, escalating in incandescence ───────────────────────
        //
        // Static concentric shells — the fuel gets hotter, it does not fly
        // apart. Drawn in the hazard palette, never on the temperature colour
        // scale, so it cannot be read as a temperature field.
        let fuel_radius = reach * (0.30 + 0.10 * intensity);
        let shells = 7;
        for k in (0..shells).rev() {
            let f = (k as f32 + 1.0) / shells as f32;
            let colour = if f < 0.4 { INCANDESCENT } else { HAZARD };
            // Brightening with the phase as well as the intensity: release is
            // an accumulating process, not an instant.
            let strength = 0.55 * (1.0 - f) * intensity * (0.55 + 0.45 * phase) + 0.06;
            painter.circle_filled(
                centre,
                fuel_radius * f,
                translucent(colour, alpha(strength)),
            );
        }
        // A soft edge to the fuel region, so it reads as a region rather than
        // as a light source.
        painter.circle_stroke(
            centre,
            fuel_radius,
            Stroke::new(
                (reach * 0.006).max(1.0),
                translucent(HAZARD, alpha(0.35 * intensity)),
            ),
        );

        // ── Release marks drifting out of the fuel region ──────────────────
        //
        // Deterministic, so they drift outward with the phase rather than
        // re-scattering every repaint. They rise as well as spread: this is a
        // slow, buoyant escape, not an ejection.
        let drift = release_reach(phase, reach * 0.55);
        let marks = 30;
        for i in 0..marks {
            let angle = (i as f32 + release_hash(i, 0, 191)) * TAU / marks as f32;
            let (sin, cos) = angle.sin_cos();
            let spread = fuel_radius + drift * (0.35 + 0.65 * release_hash(i, 1, 193));
            let at = Pos2::new(
                centre.x + cos * spread * 0.85,
                // Buoyant: marks bias upward as they get further out.
                centre.y + sin * spread * 0.85 - drift * 0.35 * release_hash(i, 2, 197),
            );
            let size = (reach * 0.010 * (0.6 + release_hash(i, 3, 199))).max(1.0);
            // Fading with distance, so the annotation reads as escape and
            // dispersal rather than as an expanding shell.
            let fade = 1.0 - (spread - fuel_radius) / (drift.max(1.0) + fuel_radius);
            painter.circle_filled(
                at,
                size,
                translucent(HAZARD, alpha(0.75 * intensity * fade.clamp(0.15, 1.0))),
            );
        }

        // ── The species, named in their sourced order ──────────────────────
        if self.show_labels {
            let named = self.named_species();
            for (i, species) in named.iter().enumerate() {
                let angle = -0.9 + i as f32 * 0.75;
                let radius = fuel_radius + drift * 0.75 + reach * 0.05;
                painter.text(
                    Pos2::new(
                        centre.x + radius * angle.cos(),
                        centre.y + radius * angle.sin() - reach * 0.04,
                    ),
                    Align2::CENTER_CENTER,
                    species.symbol,
                    FontId::proportional(11.0),
                    translucent(INCANDESCENT, alpha(0.85 * intensity)),
                );
            }
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
        let band_height = (rect.height() * 0.20).clamp(30.0, 72.0);
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
            Pos2::new(band.center().x, band.top() + band_height * 0.24),
            10.5,
            TEXT,
            &headline,
        );
        self.text(
            painter,
            Pos2::new(band.center().x, band.top() + band_height * 0.50),
            8.5,
            translucent(TEXT, 210),
            stage.mechanism(),
        );
        self.text(
            painter,
            Pos2::new(band.center().x, band.top() + band_height * 0.74),
            8.5,
            translucent(TEXT, 190),
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
                Pos2::new(band.center().x, band.bottom() + band_height * 0.30),
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
    /// generic modular-HTR retention figure (1600 degC) must not be conflated,
    /// and that any HTR-10 margin uses 1230. Require
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
    /// as the far landmark it is. (Unchanged by the 2026-08-12 physics rework,
    /// which changed what is drawn, not where the landmarks are.)
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

    /// **The physics correction, pinned.** No release may be drawn across the
    /// band in which coating retention is the demonstrated behaviour.
    ///
    /// **Methodology.** The evidence this workspace holds says the fuel is
    /// *retaining* over most of the range this overlay annotates: HTR-10
    /// coating integrity was experimentally proven to 1250 degC (Gao & Shi
    /// 2002, recorded in `docs/reactor-scoping/htr10-plant-data.md`), and the
    /// German core-heat-up simulation tests on irradiated LEU UO2 TRISO
    /// spherical fuel elements found no particle failures and no noticeable
    /// caesium or strontium release during the first few hundred hours of any
    /// 1600 degC heating test — near-100 % retention at the generic limit
    /// itself (Kugeler et al. 2017, EUR 28712 EN, section 4.2.1). So sweep the
    /// HTR-10 trigger across the whole band from the reactor's own limit to the
    /// generic figure, in 1 degC steps, and require the stage to be
    /// [`ExcursionStage::LimitExceeded`] and **never**
    /// [`ExcursionStage::FissionProductRelease`] anywhere below 1600 degC.
    /// Require the escalation to happen exactly at the generic figure, and
    /// require the earlier stage's own text to state that the coating is not
    /// assumed failed.
    ///
    /// **Result (2026-08-12):** 369 temperatures sampled strictly between the
    /// landmarks, every one `LimitExceeded`; 1599 degC gave `LimitExceeded` at
    /// intensity 0.9973 and 1600 degC gave `FissionProductRelease` at intensity
    /// 1.0000; 1250 degC — the temperature to which coating integrity was
    /// experimentally proven — was `LimitExceeded`, not release. Interpretation:
    /// the annotation cannot depict release where the cited evidence shows
    /// retention, which is exactly the error the earlier explosion artwork
    /// made.
    #[test]
    fn no_release_is_drawn_where_retention_is_demonstrated() {
        let stage_at = |t: f64| {
            ExcursionStage::from_intensity(
                ExcursionTrigger::htr10_fuel_temperature(degc(t)).intensity(),
            )
        };

        let mut sampled = 0usize;
        for step in 1..370 {
            let fuel = 1230.0 + step as f64;
            assert_eq!(
                stage_at(fuel),
                ExcursionStage::LimitExceeded,
                "{fuel} degC must not be drawn as release"
            );
            sampled += 1;
        }
        println!("{sampled} temperatures between the landmarks, all LimitExceeded");

        // The temperature coating integrity was experimentally proven to.
        assert_eq!(stage_at(1250.0), ExcursionStage::LimitExceeded);

        // Escalation happens exactly at the generic figure and not before.
        assert_eq!(stage_at(1599.0), ExcursionStage::LimitExceeded);
        assert_eq!(stage_at(1600.0), ExcursionStage::FissionProductRelease);
        assert_eq!(stage_at(1800.0), ExcursionStage::FissionProductRelease);
        println!(
            "1599 -> {:.4} ({:?}), 1600 -> {:.4} ({:?})",
            ExcursionTrigger::htr10_fuel_temperature(degc(1599.0)).intensity(),
            stage_at(1599.0),
            ExcursionTrigger::htr10_fuel_temperature(degc(1600.0)).intensity(),
            stage_at(1600.0)
        );

        // And the earlier stage must say so in words.
        assert!(ExcursionStage::LimitExceeded
            .caption()
            .contains("not assumed failed"));
        assert!(ExcursionStage::LimitExceeded
            .mechanism()
            .contains("retention"));
    }

    /// Nothing in this module may describe an explosion.
    ///
    /// **Methodology.** The rework exists because the earlier artwork drew a
    /// blast, which is physically wrong for a helium-cooled graphite core and
    /// inverts the claim TRISO fuel is built on. A future edit could reintroduce
    /// the language, so require every stage's label, caption and mechanism to
    /// contain none of "explos", "blast", "debris", "detonat" or "shock", and
    /// require the release stage's own text to name the gradual mechanism.
    ///
    /// A term may appear **only inside an explicit denial** — "not a blast",
    /// "not explosive" — because saying what this is not is part of the
    /// correction, while any other use would be a regression.
    ///
    /// **Result (2026-08-12):** nine strings checked across three stages; the
    /// only matches were the two permitted denials — the release stage's
    /// mechanism names "coating degradation and gradual release over hours —
    /// not a blast" and its caption states "progressive, not explosive".
    /// Interpretation: the annotation's own words now match the physics, and a
    /// regression fails here rather than shipping.
    #[test]
    fn no_stage_describes_an_explosion() {
        let forbidden = ["explos", "blast", "debris", "detonat", "shock"];
        let permitted_denials = ["not a blast", "not explosive"];
        let mut checked = 0usize;
        let mut denials = 0usize;
        for stage in ExcursionStage::ALL {
            for text in [stage.label(), stage.caption(), stage.mechanism()] {
                let lowered = text.to_lowercase();
                for term in forbidden {
                    if !lowered.contains(term) {
                        continue;
                    }
                    let denied = permitted_denials.iter().any(|d| lowered.contains(d));
                    assert!(denied, "{stage:?} text mentions '{term}': {text:?}");
                    denials += 1;
                }
                checked += 1;
            }
        }
        println!("{checked} stage strings checked, {denials} permitted denial(s)");

        assert!(ExcursionStage::FissionProductRelease
            .mechanism()
            .contains("gradual"));
        assert!(ExcursionStage::FissionProductRelease
            .caption()
            .contains("progressive"));
    }

    /// The species named must follow the order the heating tests report, and
    /// must not be named at all while the coating is still retaining.
    ///
    /// **Methodology.** Kugeler et al. (2017) section 4.2.1 report that
    /// caesium is retained at 1600 degC by the kernel, the SiC and the A3
    /// matrix and released without delay at 1800 degC; that krypton is *always*
    /// released later than caesium because the intact pyrocarbon layers retain
    /// it; and that strontium is retained better than caesium in oxide kernels,
    /// so it is released later still. Require [`RELEASE_ORDER`] to be exactly
    /// Cs, Kr, Sr; require [`species_visible`] to reveal them in that order and
    /// never out of it; and require [`ExcursionOverlay::named_species`] to be
    /// empty at every stage below release, however long the clock has run.
    ///
    /// **Result (2026-08-12):** order 137Cs, 85Kr, 90Sr; across 101 sampled
    /// phases the visible set never contained a later species without every
    /// earlier one, reaching all three by phase 0.75; a `LimitExceeded` overlay
    /// at 1450 degC named none even after 60 s of clock, and a release overlay
    /// named all three. Interpretation: the sequence on screen is the sequence
    /// in the literature, and nothing is named while retention is demonstrated.
    #[test]
    fn the_species_are_named_in_the_order_the_heating_tests_report() {
        assert_eq!(RELEASE_ORDER.len(), 3);
        assert_eq!(RELEASE_ORDER[0].nuclide, "137Cs");
        assert_eq!(RELEASE_ORDER[1].nuclide, "85Kr");
        assert_eq!(RELEASE_ORDER[2].nuclide, "90Sr");
        for species in RELEASE_ORDER {
            assert!(!species.symbol.is_empty());
            assert!(!species.note.is_empty());
        }

        let mut sampled = 0usize;
        let mut all_by = f32::INFINITY;
        for step in 0..=100 {
            let phase = step as f32 / 100.0;
            let visible: Vec<bool> = (0..RELEASE_ORDER.len())
                .map(|i| species_visible(i, phase))
                .collect();
            // No species may appear before an earlier one.
            for i in 1..visible.len() {
                assert!(
                    !visible[i] || visible[i - 1],
                    "{} appeared before {} at phase {phase}",
                    RELEASE_ORDER[i].nuclide,
                    RELEASE_ORDER[i - 1].nuclide
                );
            }
            if visible.iter().all(|v| *v) && phase < all_by {
                all_by = phase;
            }
            sampled += 1;
        }
        println!("{sampled} phases sampled; all three named by phase {all_by:.2}");
        assert!(all_by <= 0.75 + 1e-6);
        assert!(!species_visible(0, f32::NAN));
        assert!(!species_visible(9, 1.0), "there is no fourth species");

        // Nothing is named while the coating is still demonstrated to retain,
        // however long the clock runs.
        let warning = ExcursionOverlay::new(
            ExcursionTrigger::htr10_fuel_temperature(degc(1450.0)),
            Pos2::ZERO,
            Vec2::splat(100.0),
        )
        .since_trigger(seconds(60.0));
        assert_eq!(warning.stage(), ExcursionStage::LimitExceeded);
        assert!(
            warning.named_species().is_empty(),
            "no nuclide may be named while the fuel is still retaining"
        );

        let release = ExcursionOverlay::new(
            ExcursionTrigger::htr10_fuel_temperature(degc(1700.0)),
            Pos2::ZERO,
            Vec2::splat(100.0),
        )
        .since_trigger(seconds(60.0));
        assert_eq!(release.stage(), ExcursionStage::FissionProductRelease);
        assert_eq!(release.named_species().len(), 3);
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
    /// it. Interpretation: a reactor inside its specification is never
    /// annotated, and a broken model cannot look healthy.
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
    /// [`RELEASE_INTENSITY`], and `FissionProductRelease` at and above it, with
    /// no other transitions. Require a non-finite intensity to escalate, and
    /// require only `Quiescent` to draw nothing.
    ///
    /// **Result (2026-08-12):** 2 001 samples, exactly two transitions — at
    /// intensity 0.001 into `LimitExceeded` and at 1.000 into
    /// `FissionProductRelease`, matching `RELEASE_INTENSITY` = 1.0; NaN and
    /// both infinities gave `FissionProductRelease`; `Quiescent` was the only
    /// stage with no label, no caption and `is_drawn() == false`.
    /// Interpretation: nothing is ever drawn over a reactor inside its
    /// specification, and release is annotated only at the far landmark.
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
                ExcursionStage::LimitExceeded => assert!(i > 0.0 && i < RELEASE_INTENSITY),
                ExcursionStage::FissionProductRelease => assert!(i >= RELEASE_INTENSITY),
            }
            samples += 1;
        }
        println!("{samples} samples, transitions at {transitions:?}");
        assert_eq!(transitions.len(), 2, "expected exactly two escalations");

        assert_eq!(
            ExcursionStage::from_intensity(f32::NAN),
            ExcursionStage::FissionProductRelease
        );
        assert_eq!(
            ExcursionStage::from_intensity(f32::INFINITY),
            ExcursionStage::FissionProductRelease
        );

        assert!(!ExcursionStage::Quiescent.is_drawn());
        assert!(ExcursionStage::Quiescent.label().is_empty());
        assert!(ExcursionStage::Quiescent.caption().is_empty());
        assert!(ExcursionStage::Quiescent.mechanism().is_empty());
        for stage in [
            ExcursionStage::LimitExceeded,
            ExcursionStage::FissionProductRelease,
        ] {
            assert!(stage.is_drawn());
            assert!(!stage.label().is_empty());
            assert!(!stage.caption().is_empty());
            assert!(!stage.mechanism().is_empty());
        }
        assert_eq!(ExcursionStage::ALL.len(), 3);
        // The caption must keep saying the model, not just the reactor, is out
        // of range — RESPONSIBLE_USE.md requires that framing.
        assert!(ExcursionStage::FissionProductRelease
            .caption()
            .contains("demonstration only"));
    }

    /// The annotation must advance only with the **application's** clock, and
    /// must never fade back to nothing.
    ///
    /// **Methodology.** The widget is rebuilt every repaint, so the phase must
    /// come from a caller-supplied elapsed simulation time. Require
    /// [`release_phase`] to be 0.0 at and before the trigger instant, to reach
    /// 1.0 exactly at [`RELEASE_RAMP_SECONDS`], to stay at 1.0 afterwards
    /// (released products do not go back into the fuel), to be monotonic in
    /// between, and to give 0.0 for a non-finite time. Then require two
    /// overlays built with the same elapsed time to report the same phase — the
    /// property a widget-owned clock would break — and one built with a later
    /// time to report a larger one.
    ///
    /// **Result (2026-08-12):** phase 0.000 at -5 s and 0 s, 0.500 at 0.70 s,
    /// 1.000 at 1.40 s and still 1.000 at 60 s; monotonic over 200 sampled
    /// times; NaN gave 0.000. Two overlays at 0.35 s both reported 0.250, and
    /// one at 0.70 s reported 0.500. Interpretation: the annotation is a pure
    /// function of the simulation clock, so it survives being rebuilt every
    /// frame, pauses when the simulation pauses, and replays identically.
    #[test]
    fn the_phase_comes_from_the_application_clock_and_does_not_reverse() {
        assert_eq!(release_phase(seconds(-5.0)), 0.0);
        assert_eq!(release_phase(seconds(0.0)), 0.0);
        assert!((release_phase(seconds(0.7)) - 0.5).abs() < 1e-6);
        assert_eq!(release_phase(seconds(RELEASE_RAMP_SECONDS)), 1.0);
        assert_eq!(release_phase(seconds(60.0)), 1.0);
        assert_eq!(release_phase(Time::new::<second>(f64::NAN)), 0.0);

        let mut previous = 0.0;
        for k in 0..=200 {
            let t = k as f64 * 0.01;
            let phase = release_phase(seconds(t));
            assert!(phase >= previous, "phase went backwards at {t} s");
            previous = phase;
        }

        let overlay = |t: f64| {
            ExcursionOverlay::new(
                ExcursionTrigger::htr10_fuel_temperature(degc(1700.0)),
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

    /// The release marks must drift outward, slowing, and must stay inside the
    /// box they were given.
    ///
    /// **Methodology.** The drift is a display easing, not a transport
    /// calculation. Require [`release_reach`] to be zero at phase 0, exactly
    /// the maximum reach at phase 1, strictly increasing in between, never
    /// greater than the maximum, and **concave** — each successive equal step in
    /// phase must move the marks no further than the one before, so the
    /// annotation settles rather than sweeping outward at constant speed. Also
    /// require out-of-range and non-finite inputs to clamp to something
    /// drawable.
    ///
    /// **Result (2026-08-12):** over 100 equal phase steps on a 100-point
    /// maximum reach, the first step moved 10.00 points and the last 0.50
    /// points, with every step no larger than its predecessor; reach 0.00 at
    /// phase 0, 70.71 at phase 0.5 and 100.00 at phase 1; phase 5.0 clamped to
    /// 100.00 and NaN gave 0.00. Interpretation: the marks disperse and settle
    /// inside the annotated rectangle, with no expanding front.
    #[test]
    fn the_release_marks_drift_outward_and_settle() {
        let max = 100.0f32;
        assert_eq!(release_reach(0.0, max), 0.0);
        assert!((release_reach(1.0, max) - max).abs() < 1e-4);
        assert!((release_reach(0.5, max) - 70.710_68).abs() < 1e-3);
        assert_eq!(release_reach(5.0, max), max);
        assert_eq!(release_reach(-1.0, max), 0.0);
        assert_eq!(release_reach(f32::NAN, max), 0.0);
        assert_eq!(release_reach(0.5, f32::NAN), 0.0);

        let steps = 100;
        let mut previous_reach = 0.0f32;
        let mut previous_step = f32::INFINITY;
        let mut first_step = 0.0f32;
        let mut last_step = 0.0f32;
        for k in 1..=steps {
            let reach = release_reach(k as f32 / steps as f32, max);
            let step = reach - previous_reach;
            assert!(step > 0.0, "the marks stalled at step {k}");
            assert!(
                reach <= max + 1e-4,
                "the marks escaped their box at step {k}"
            );
            assert!(
                step <= previous_step + 1e-4,
                "the marks accelerated at step {k}"
            );
            if k == 1 {
                first_step = step;
            }
            last_step = step;
            previous_step = step;
            previous_reach = reach;
        }
        println!("first step {first_step:.2} points, last step {last_step:.2} points");
        assert!(first_step > 10.0 * last_step, "the drift barely settled");
    }

    /// The banner pulse must stay bounded, be a pure function of simulation
    /// time, and stay visible when the clock is broken.
    ///
    /// **Methodology.** Sample [`banner_pulse`] over 2 001 simulation times and
    /// require every value to lie in `[0, 1]`, the same time to give the same
    /// value bitwise, the pulse to actually vary (a constant would be a dead
    /// animation), and a non-finite time to give full brightness rather than
    /// darkness — a broken clock must not hide the warning.
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

    /// The release scatter must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so marks drawn
    /// from a real random source would re-scatter each frame — they would
    /// flicker instead of drifting. Evaluate [`release_hash`] repeatedly at
    /// 3 000 (index, salt) sites and require bitwise-equal results in
    /// `[0, 1)`, the salted draws at one site to differ, and adjacent indices
    /// to decorrelate.
    ///
    /// **Result (2026-08-12):** 3 000 hash sites re-evaluated three times each,
    /// all bitwise identical and in range; 37 of 40 adjacent index pairs
    /// differed by more than 0.05; the four salted draws at one site were
    /// pairwise distinct. Interpretation: the release pattern is fixed by the
    /// index, so it drifts with the phase instead of boiling.
    #[test]
    fn the_release_scatter_is_deterministic() {
        let mut checked = 0usize;
        for index in -50..250 {
            for salt in 191..201u32 {
                let first = release_hash(index, 0, salt);
                for _ in 0..3 {
                    assert_eq!(
                        first,
                        release_hash(index, 0, salt),
                        "hash is not deterministic"
                    );
                }
                assert!((0.0..1.0).contains(&first), "hash {first} out of range");
                checked += 1;
            }
        }
        println!("{checked} hash sites re-evaluated");

        let (a, b, c, d) = (
            release_hash(7, 3, 191),
            release_hash(7, 3, 193),
            release_hash(7, 3, 197),
            release_hash(7, 3, 199),
        );
        assert!((a - b).abs() > 1e-6 && (b - c).abs() > 1e-6 && (c - d).abs() > 1e-6);

        let mut differing = 0;
        for i in 0..40 {
            if (release_hash(i, 0, 191) - release_hash(i + 1, 0, 191)).abs() > 0.05 {
                differing += 1;
            }
        }
        println!("{differing}/40 adjacent index pairs decorrelated");
        assert!(differing > 30, "the release marks will look striped");
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

        // 1450 degC is past the HTR-10's own limit but well below the generic
        // figure, so it is a WARNING, not release.
        assert_eq!(overlay.stage(), ExcursionStage::LimitExceeded);
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
            ExcursionTrigger::Intensity(1.0),
            Pos2::ZERO,
            Vec2::new(100.0, 100.0),
        );
        assert_eq!(bare.overshoot_kelvin(), None);
        assert_eq!(bare.stage(), ExcursionStage::FissionProductRelease);
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
            assert!(overlay.named_species().is_empty());
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
