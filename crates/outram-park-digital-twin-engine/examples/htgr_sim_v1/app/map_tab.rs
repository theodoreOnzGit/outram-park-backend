//! **Map** -- Gaussian puff atmospheric dispersion around the plant.
//!
//! Since 2026-09-22 (maintainer direction) this tab holds only the dispersion
//! widgets: the live core map, the pebble and TRISO particle drill-down and
//! the release table moved to [`super::thermal_tab`] ("Live thermal state and
//! FP release"), which replaced the static geometry viewer.
//!
//! # The picture is a live plume, evaluated once per screen pixel
//!
//! Two maintainer directions, both 2026-09-25, shape what this draws:
//!
//! - *"fill the map with single pixels, not the big boxes"* -- the field is
//!   requested at the map square's own width in **physical screen pixels**,
//!   so every pixel painted is a separate evaluation of the puff model at that
//!   pixel's coordinates. It is uploaded as one texture rather than as tens of
//!   thousands of filled rectangles, which is a rendering choice and changes
//!   no value.
//! - *"timestep according to real-time, I want to see a real-time plume"* --
//!   the field is the **instantaneous** concentration at the plume clock, so
//!   it grows out of the stack, travels downwind, and settles after one puff
//!   lifetime. See [`crate::physics::atmospheric_dispersion::DispersionGrid`],
//!   which spells out why that is a *different quantity* from the
//!   time-integrated `chi/Q` the table below quotes, despite sharing units.
//!
//! The fast-forward buttons move the **plume** clock only. The plant clock is
//! shown beside it and is never advanced by them -- see
//! [`crate::physics::atmospheric_dispersion::MapFieldRequest::plume_clock_offset`]
//! for why the plume may legitimately run ahead and what that must never be
//! read as.
//!
//! **Since gh:#344 the field is a MARCHED Lagrangian puff population**, not a
//! closed form in `(meteorology, clock)`. Two consequences are stated at the
//! controls that cause them, because they change what a fast-forward may be
//! read as: a forward jump is marched under the **current** wind, so it is an
//! extrapolation under "this wind holds" rather than an exact evaluation at a
//! later argument; and a **rewind clears the population**, because the march
//! is not invertible and no per-step history is kept. ~~(Note for the next
//! reader: `MapFieldRequest::plume_clock_offset`'s own doc comment still
//! carries the superseded "exact at any offset" wording. [...] reported rather
//! than fixed, 2026-09-27.)~~ **CORRECTED 2026-09-28** -- re-checked: that
//! doc comment now strikes the "exact at any offset" wording and carries the
//! two caveats, so the note no longer applies.
//!
//! # What the map paints, and in which unit (maintainer direction, 2026-09-28)
//!
//! The physics field is the instantaneous `chi/Q` \[s/m^3\]. The map multiplies
//! it by a release rate and paints the product, on one of three bases chosen
//! by a toggle ([`MapBasis`]):
//!
//! | Basis | Painted quantity | Unit |
//! |---|---|---|
//! | **Absolute** (default) | instantaneous air concentration, 5 tracked nuclides | Bq/m^3 |
//! | Per Ci | the same, per curie of core inventory | Bq/m^3 per Ci |
//! | `chi/Q` | the source-independent dilution factor (the pre-2026-09-28 map) | s/m^3 |
//!
//! | **Dose rate** (since 2026-09-29) | INDICATIVE effective dose rate, air pathways | µSv/h |
//!
//! Absolute falls back to per-Ci when the release channel has no absolute arm.
//! Dose rate never falls back: without the absolute arm it is unavailable and
//! says so.
//!
//! ## The dose-rate basis (maintainer direction, 2026-09-29)
//!
//! *"wire in buangkok into the map … additional option for dose rate … The
//! scaling should be 0.8 microsieverts/hr (just about airline) all the way up
//! to 1 mSv/hr"*, and *"i want a table for dose rates (microsieverts/hr),
//! placed above the absolute basis map"*. The physics is in
//! [`crate::physics::dose_rate`], through `buangkok`'s pathway functions and
//! its shipped US EPA coefficients (FGR-15 2025, FGR-11): a pixel is cloud
//! submersion + committed inhalation from the live air concentration; ground
//! shine from the dry deposit is in the receptor table only (the map has no
//! per-pixel deposit). **Indicative dose rate, research/education only, not a
//! dose to any real person, not for emergency or regulatory use** -- said on
//! screen next to the control and the table. ~~"nothing on this tab computes
//! or displays a dose (dose belongs to `buangkok`)"~~ **CHANGED 2026-09-29**:
//! the dose-rate basis displays one, computed by `buangkok`.
//!
//! The colour scale is set by two sliders -- a floor and a span in decades --
//! whose absolute-basis defaults are the **indicative** anchors
//! [`banana_anchor`] and [`one_sievert_anchor`], derived from US EPA
//! Federal Guidance Report No. 11. **They are colour-scale anchors, not a dose
//! calculation**: ~~nothing on this tab computes or displays a dose (dose belongs
//! to `buangkok`)~~ (**CHANGED 2026-09-29**: the Dose-rate basis displays an
//! indicative dose rate, computed by `buangkok` -- see below), and the anchors are activities of intake placed numerically
//! on a Bq/m^3 scale, which is not a unit conversion. See
//! [`FGR11_K40_INGESTION_SV_PER_BQ`] for the derivation.
//!
//! # NOT VALIDATED
//!
//! The puff model is `changi::puff` (ported from R `puff` 0.1.1), not FLEXPART,
//! and it is driven by the release the thermal tab shows. The absolute basis is
//! the release channel's absolute arm (Liu & Cao 2002 inventory through
//! TRISO-ATOPS reference failure fractions and a primary-circuit leak) and is
//! **not a source term**. See [`crate::physics::atmospheric_dispersion`].

use egui::{
    Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
    Ui, Vec2,
};

use outram_park_digital_twin_engine::app_scaffold::SharedState;
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;

use super::state::HtgrSnapshot;
use crate::physics::dose_rate::{self, Pathway};
use crate::physics::atmospheric_dispersion::FieldWeighting;
use crate::physics::fission_product_release::TRACKED_NUCLIDES;

/// Fraction of the tab's height the map square takes.
///
/// **Maintainer direction, 2026-09-25: "make the map fill like 60% of the
/// height"**, with the dispersion table below it rather than beside it. A
/// drawing-layout constant with no physical counterpart, recorded here as
/// their call so the next reader does not "fix" it back to a square that
/// fits whatever is left.
const MAP_HEIGHT_FRACTION: f32 = 0.60;

/// Floor on the map square's side \[points\].
///
/// A window short enough that 60 % of it is smaller than this gets a map that
/// overflows into the tab's two-way scroll area instead of collapsing to an
/// unreadable thumbnail. Purely a drawing choice.
const MAP_MIN_SIDE: f32 = 320.0;

/// Steps the plume-clock fast-forward offers \[s of plume clock\].
///
/// **Maintainer direction, 2026-09-25: "timesteps of 1-2 hrs at a time".**
/// One and two hours, plus a one-hour rewind.
///
/// ~~"plus a one-hour rewind so a jump can be walked back without
/// restarting"~~ **CORRECTED 2026-09-27.** That claim shaped this control and
/// is false since gh:#344 made the field a marched Lagrangian puff population:
/// a rewind DOES restart the plume. `advance_population` on
/// `AtmosphericDispersionChannel` clears the population and re-marches from
/// the stack,
/// because the trajectory integral is not invertible and no per-step history
/// is kept. The button stays -- the clock really does go back -- but it is a
/// restart, and the hover text says so.
const PLUME_JUMPS_S: [(f64, &str); 3] = [(3600.0, "+1 h"), (7200.0, "+2 h"), (-3600.0, "-1 h")];

/// US EPA **Federal Guidance Report No. 11** committed effective dose
/// equivalent per unit intake for **K-40, ingestion**, adult \[Sv/Bq\]:
/// **5.02 x 10^-9**.
///
/// # Source (DATA_POLICY provenance)
///
/// K. F. Eckerman, A. B. Wolbarst and A. C. B. Richardson, *Limiting Values
/// of Radionuclide Intake and Air Concentration and Dose Conversion Factors for
/// Inhalation, Submersion, and Ingestion*, Federal Guidance Report No. 11,
/// EPA-520/1-88-020, US EPA, 1988. **Table 2.2** "Exposure-to-Dose Conversion
/// Factors for Ingestion", row K-40 (f1 = 1.0), column **Effective**; printed
/// page **156** (PDF page 164). A US government report, freely distributed by
/// epa.gov (`https://www.epa.gov/sites/default/files/2015-05/documents/520-1-88-020.pdf`),
/// accessed 2026-09-28; the value was read off the rendered table page, not
/// typed from memory (the PDF's OCR text layer is unreliable for exponents).
///
/// # Why FGR-11 and not FGR-13
///
/// FGR-13 (EPA 402-R-99-001, 1999) was also checked (the maintainer's local
/// copy, read with `pdftotext -layout`): its Chapter 2 tables are **cancer
/// risk coefficients per Bq** -- e.g. Table 2.2a "Mortality and morbidity risk
/// coefficients for ingestion of water and food", printed page 84 (PDF page
/// 101), K-40 tap-water mortality 4.30e-10 **per Bq** -- not dose per unit
/// intake. A risk coefficient is not Sv/Bq, and no Sv/Bq table for K-40 or
/// Cs-137 was found in the report text, so FGR-13 could not supply the
/// anchor. FGR-11's coefficients are for ICRP-30 "Reference Man"
/// (adult, occupational basis) -- adequate for an **indicative colour anchor**
/// and nothing more.
///
/// # What it is used for, and what it is NOT
///
/// Only to place [`banana_anchor`]. **No dose is computed or displayed from the
/// map.** Research, education and V&V only (`RESPONSIBLE_USE.md`).
pub const FGR11_K40_INGESTION_SV_PER_BQ: f64 = 5.02e-9;

/// FGR-11 **Cs-137, ingestion**, adult, committed effective dose equivalent
/// per unit intake \[Sv/Bq\]: **1.35 x 10^-8**.
///
/// Same report as [`FGR11_K40_INGESTION_SV_PER_BQ`]: **Table 2.2, Cont'd.**,
/// row Cs-137 (f1 = 1.0), column **Effective**, printed page **166** (PDF page
/// 174), accessed 2026-09-28, read off the rendered page.
///
/// Cs-137 ingestion is the one representative nuclide/pathway chosen for the
/// top anchor: it is one of the five nuclides this simulator tracks and the
/// long-lived one a map of deposited activity is usually read against. Another
/// nuclide or pathway would move [`one_sievert_anchor`] by its coefficient
/// ratio -- which is exactly why the anchor is labelled indicative.
pub const FGR11_CS137_INGESTION_SV_PER_BQ: f64 = 1.35e-8;

/// The "banana equivalent dose" convention, **0.1 µSv** \[Sv\].
///
/// **Not from FGR-11** -- it is the informal public-communication convention
/// the maintainer named as the anchor (2026-09-28, "~15 Bq of K-40, ~0.1 µSv").
/// Recorded as that, not as a measurement.
pub const BANANA_EQUIVALENT_DOSE_SV: f64 = 1.0e-7;

/// One sievert \[Sv\], the top anchor's nominal level.
pub const ONE_SIEVERT_SV: f64 = 1.0;

/// **"≈ banana"** colour-scale floor \[Bq\]:
/// `BANANA_EQUIVALENT_DOSE_SV / FGR11_K40_INGESTION_SV_PER_BQ`
/// = 1.0e-7 / 5.02e-9 = **19.92 Bq** of K-40.
///
/// # The two halves of the maintainer's anchor disagree, and the derived one is used
///
/// The request gave both "~15 Bq of K-40" and "~0.1 µSv". Through FGR-11's
/// coefficient those are not the same point: 15 Bq x 5.02e-9 Sv/Bq = 7.5e-8 Sv,
/// and 0.1 µSv / 5.02e-9 = 19.9 Bq. The anchor is **derived from the cited
/// coefficient** (the instruction), so it is 19.9 Bq, and the ~25 % gap to
/// "~15 Bq" is reported rather than tuned away. On a log colour scale spanning
/// 6.6 decades it is 0.12 decade.
///
/// Indicative colour anchor only -- not a dose calculation.
pub fn banana_anchor() -> f64 {
    BANANA_EQUIVALENT_DOSE_SV / FGR11_K40_INGESTION_SV_PER_BQ
}

/// **"≈ 1 Sv-equivalent"** colour-scale top \[Bq\]:
/// `ONE_SIEVERT_SV / FGR11_CS137_INGESTION_SV_PER_BQ` = 1 / 1.35e-8 =
/// **7.407e7 Bq** of Cs-137 ingested.
///
/// Indicative colour anchor only -- not a dose calculation.
pub fn one_sievert_anchor() -> f64 {
    ONE_SIEVERT_SV / FGR11_CS137_INGESTION_SV_PER_BQ
}

/// **"≈ airliner at cruise altitude"** default floor of the dose-rate basis
/// \[µSv/h\] -- the maintainer's figure (2026-09-29), not a cited value.
pub use crate::physics::dose_rate::FLOOR_USV_PER_H;
/// **"high"** default top of the dose-rate basis \[µSv/h\] -- the
/// maintainer's figure (2026-09-29).
pub use crate::physics::dose_rate::TOP_USV_PER_H;

/// The framing every dose-rate readout carries (hard rule,
/// `crates/buangkok/CLAUDE.md`). Pinned by a test so it cannot be edited away.
pub const DOSE_RATE_FRAMING: &str =
    "INDICATIVE dose rate -- research/education only, not a dose to \
     any real person, not for emergency or regulatory use.";

/// **Reference figure, not a threshold or zone:** the NRC's 2023 plume-exposure
/// EPZ sizing criterion for small modular reactors and other new
/// technologies, **10 mSv (1 rem) TEDE over 96 hours** \[Sv\] -- 10 CFR
/// 50.33(g)(2)(i)(A), final rule "Emergency Preparedness for Small Modular
/// Reactors and Other New Technologies", 88 FR 80050 (16 Nov 2023): the rule
/// text at 88 FR 80074 and the preamble at 88 FR 80058 ("projected to exceed
/// 10 millisieverts (mSv) (1 rem) total effective dose equivalent (TEDE) over
/// 96 hours from the release"), as enclosed in NRC STC-23-079
/// (`theodore-open-corpus/hjg2023date.pdf`, PDF pp. 12 and 28).
///
/// Shown on the Dose-rate basis purely as a **comparison figure**: this map is
/// indicative, research/education only, and must not be read as emergency-
/// planning support (`RESPONSIBLE_USE.md`; `changi`'s scope). The criterion is
/// a projected dose to the public over a spectrum of accidents; the map is an
/// indicative instantaneous dose rate from this simulator's own release.
pub const NRC_2023_EPZ_CRITERION_DOSE_SV: f64 = 10.0e-3;

/// The criterion's integration time \[h\]: 96 hours (same source).
pub const NRC_2023_EPZ_CRITERION_HOURS: f64 = 96.0;

/// The criterion as an **average** dose rate \[µSv/h\]:
/// `10 mSv / 96 h` = **104.17 µSv/h**. It assumes a constant rate over the
/// whole 96 hours; a front-loaded release (the usual case) reaches 10 mSv
/// sooner, so a rate below this line can still exceed the criterion's dose,
/// and a rate above it need not. A reference marker on the colour scale only.
pub fn nrc_2023_epz_reference_usv_per_h() -> f64 {
    NRC_2023_EPZ_CRITERION_DOSE_SV / NRC_2023_EPZ_CRITERION_HOURS * 1.0e6
}

/// The label the reference marker carries, everywhere it is drawn.
pub const NRC_2023_EPZ_REFERENCE_LABEL: &str = "10 mSv / 96 h average (NRC 2023 EPZ sizing \
     criterion, 88 FR 80050) -- reference only, not a zone or threshold";

/// The dose-rate basis's default scale: floor [`FLOOR_USV_PER_H`] (0.8 µSv/h),
/// top [`TOP_USV_PER_H`] (1 mSv/h), span `log10(1000 / 0.8)` = 3.097 decades.
pub fn dose_rate_anchor_scale() -> ColourScale {
    ColourScale::clamped(FLOOR_USV_PER_H, (TOP_USV_PER_H / FLOOR_USV_PER_H).log10())
}

/// Which quantity the map paints. See the module doc's table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapBasis {
    /// Instantaneous air concentration \[Bq/m^3\], absolute basis. Default.
    #[default]
    Absolute,
    /// Instantaneous air concentration per curie of core inventory
    /// \[Bq/m^3 per Ci\].
    PerCi,
    /// The source-independent dilution factor `chi/Q` \[s/m^3\] -- the map as
    /// it was before 2026-09-28, kept because it is the one quantity here that
    /// is not hostage to a source-term input, and because it still draws a
    /// plume when the release channel is empty.
    ChiOverQ,
    /// INDICATIVE effective dose rate \[µSv/h\] from the air pathways
    /// (submersion + committed inhalation), through `buangkok`; see
    /// [`crate::physics::dose_rate`]. Not a dose to any real person.
    DoseRate,
}

impl MapBasis {
    const ALL: [MapBasis; 4] = [
        MapBasis::Absolute,
        MapBasis::PerCi,
        MapBasis::ChiOverQ,
        MapBasis::DoseRate,
    ];

    fn index(self) -> usize {
        match self {
            MapBasis::Absolute => 0,
            MapBasis::PerCi => 1,
            MapBasis::ChiOverQ => 2,
            MapBasis::DoseRate => 3,
        }
    }

    /// The painted quantity's true unit.
    fn unit(self) -> &'static str {
        match self {
            MapBasis::Absolute => "Bq/m^3",
            MapBasis::PerCi => "Bq/m^3 per Ci",
            MapBasis::ChiOverQ => "s/m^3",
            MapBasis::DoseRate => "µSv/h",
        }
    }

    fn label(self) -> &'static str {
        match self {
            MapBasis::Absolute => "Absolute [Bq/m^3]",
            MapBasis::PerCi => "Per Ci of core inventory [Bq/m^3 per Ci]",
            MapBasis::ChiOverQ => "chi/Q [s/m^3]",
            MapBasis::DoseRate => "Dose rate [µSv/h] (indicative)",
        }
    }

    /// The field weighting the physics must sum the map in for this basis
    /// (gh:#400): each puff at the release rate of ITS emission, not the
    /// whole plume at today's rate.
    fn weighting(self) -> FieldWeighting {
        match self {
            MapBasis::Absolute => FieldWeighting::AbsoluteBqPerM3,
            MapBasis::PerCi => FieldWeighting::PerCiBqPerM3,
            MapBasis::ChiOverQ => FieldWeighting::ChiOverQ,
            MapBasis::DoseRate => FieldWeighting::DoseRateUsvPerH,
        }
    }

    /// The CURRENT release's factor on this basis: a release rate \[Bq/s\]
    /// or \[Bq/s per Ci\], 1, or (dose rate) the air-pathway dose rate per
    /// unit `chi/Q` \[µSv/h per s/m^3\] from `buangkok`. `NAN` when the
    /// rate is not available. **Used only to tell whether a basis is
    /// available** -- ~~the texture multiplied the whole instantaneous
    /// `chi/Q` field by it~~ **CORRECTED 2026-09-29 (gh:#400, #346)**: that
    /// rescaled puffs emitted an hour ago by the release rate of now; the
    /// physics now sums each puff at its own emission's rate
    /// ([`FieldWeighting`]).
    fn factor(self, s: &HtgrSnapshot) -> f64 {
        match self {
            MapBasis::Absolute => s.dispersion_source_rate_absolute_bq_per_s,
            MapBasis::PerCi => s.dispersion_source_rate_per_ci_bq_per_s,
            MapBasis::ChiOverQ => 1.0,
            MapBasis::DoseRate => dose_rate::air_dose_rate_per_unit_chi_over_q(
                &s.dispersion_source_rate_absolute_by_nuclide_bq_per_s,
                dose_rate::coefficients(),
            ),
        }
    }
}

/// The basis actually painted: the requested one, except that **Absolute
/// falls back to Per Ci** when the absolute release rate is unavailable
/// (maintainer direction, 2026-09-28). Never falls back silently -- the
/// caller shows a label when the two differ.
fn effective_basis(requested: MapBasis, s: &HtgrSnapshot) -> MapBasis {
    if requested == MapBasis::Absolute && !s.dispersion_source_rate_absolute_bq_per_s.is_finite() {
        MapBasis::PerCi
    } else {
        requested
    }
}

/// The value the map paints for one grid sample on `basis`: the sample
/// itself when the physics summed the grid in `basis`'s weighting, `NAN`
/// (painted as nothing) for the frame or two after a basis switch before the
/// physics has re-summed it. ~~`chi/Q x the current rate`~~ **CORRECTED
/// 2026-09-29 (gh:#400)**: the grid is already in the painted unit, each puff
/// at its emission's rate, so nothing is rescaled here. The receptor tables
/// read the same emission-weighted sums per nuclide
/// (`ReceptorSnapshot::instantaneous_air_bq_per_m3_by_nuclide`).
fn field_value(sample: f64, basis: MapBasis, s: &HtgrSnapshot) -> f64 {
    if s.dispersion_grid_weighting == basis.weighting() {
        sample
    } else {
        f64::NAN
    }
}

/// The instantaneous air concentration at a receptor summed over the tracked
/// nuclides \[Bq/m^3\], each puff at its emission's rate -- the Absolute
/// pixel under the receptor. `NAN` when any nuclide's rate is unavailable.
fn receptor_air_absolute(r: &super::state::ReceptorSnapshot) -> f64 {
    r.instantaneous_air_bq_per_m3_by_nuclide.iter().sum()
}

/// Air-pathway (submersion + committed inhalation) dose rate at a receptor
/// \[µSv/h\] from its per-nuclide live air concentration -- the dose-rate
/// pixel under the receptor (the pathway sums are linear in the
/// concentration, so summing puffs then converting equals converting each
/// puff then summing).
fn receptor_air_dose_rate(r: &super::state::ReceptorSnapshot) -> f64 {
    dose_rate::air_dose_rate_per_unit_chi_over_q(
        &r.instantaneous_air_bq_per_m3_by_nuclide,
        dose_rate::coefficients(),
    )
}

/// The map's logarithmic colour scale: a **floor** (the minimum reading that
/// gets a colour) and a **span** in decades above it; the top is
/// `floor * 10^span`. Values at or below the floor are drawn in the light
/// "no reading" grey, values above the top clamp to the top colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColourScale {
    /// Minimum reading, in the map's unit.
    pub floor: f64,
    /// Additional range above the floor \[decades\].
    pub span_decades: f64,
}

impl ColourScale {
    /// Floor slider range, in the map's unit. Wide enough to hold every basis:
    /// `chi/Q` peaks near 1e-5 s/m^3 and the absolute anchors are 2e1-7e7.
    /// A drawing range, not physics.
    pub const FLOOR_MIN: f64 = 1.0e-15;
    /// See [`Self::FLOOR_MIN`].
    pub const FLOOR_MAX: f64 = 1.0e12;
    /// Span slider range \[decades\]. One decade is the least that still
    /// shows a gradient; twelve is more than the ~7-decade anchor span with
    /// margin. A drawing range, not physics.
    pub const SPAN_MIN: f64 = 1.0;
    /// See [`Self::SPAN_MIN`].
    pub const SPAN_MAX: f64 = 12.0;

    /// Build a scale, clamping both inputs into the slider ranges. A
    /// non-finite or non-positive floor goes to [`Self::FLOOR_MIN`]; a
    /// non-finite span to [`Self::SPAN_MIN`].
    pub fn clamped(floor: f64, span_decades: f64) -> Self {
        let floor = if floor.is_finite() && floor > 0.0 {
            floor.clamp(Self::FLOOR_MIN, Self::FLOOR_MAX)
        } else {
            Self::FLOOR_MIN
        };
        let span_decades = if span_decades.is_finite() {
            span_decades.clamp(Self::SPAN_MIN, Self::SPAN_MAX)
        } else {
            Self::SPAN_MIN
        };
        Self {
            floor,
            span_decades,
        }
    }

    /// `floor * 10^span`.
    pub fn top(&self) -> f64 {
        self.floor * 10f64.powf(self.span_decades)
    }

    /// Fraction up the ramp, `None` at or below the floor (or non-finite).
    fn fraction(&self, value: f64) -> Option<f64> {
        if !(value > self.floor) || !value.is_finite() {
            return None;
        }
        Some(((value / self.floor).log10() / self.span_decades).clamp(0.0, 1.0))
    }

    /// Colour for `value`.
    fn shade(&self, value: f64) -> Color32 {
        match self.fraction(value) {
            // Light, not dark: on a white ground "no reading" must recede.
            None => NO_READING_GREY,
            Some(f) => hot_to_cold_colour_mark_1(f as f32),
        }
    }
}

/// The "below the floor / no value" colour.
const NO_READING_GREY: Color32 = Color32::from_gray(225);

/// The cited anchors as a scale: floor [`banana_anchor`], top
/// [`one_sievert_anchor`], span `log10(top / floor)` = 6.570 decades.
pub fn anchor_scale() -> ColourScale {
    ColourScale::clamped(
        banana_anchor(),
        (one_sievert_anchor() / banana_anchor()).log10(),
    )
}

/// The pre-2026-09-28 convention: four decades below the field peak.
fn peak_scale(peak: f64) -> ColourScale {
    ColourScale::clamped(peak * 1.0e-4, 4.0)
}

/// The default scale for `basis` until the operator moves a slider.
///
/// - **Absolute**: the cited anchors, [`anchor_scale`].
/// - **Per Ci**: the same anchors carried onto the per-Ci basis by the ratio of
///   the two release rates (per-Ci / absolute), so a colour means the same
///   absolute concentration on both bases; when the absolute rate is not
///   available there is nothing to carry them by, and the old four-decades-
///   below-peak scale is used instead.
/// - **chi/Q**: four decades below the peak, as the map always drew it.
fn default_scale(basis: MapBasis, s: &HtgrSnapshot, field_peak: f64) -> ColourScale {
    match basis {
        MapBasis::Absolute => anchor_scale(),
        MapBasis::PerCi => {
            let (per_ci, abs) = (
                s.dispersion_source_rate_per_ci_bq_per_s,
                s.dispersion_source_rate_absolute_bq_per_s,
            );
            if per_ci.is_finite() && abs.is_finite() && per_ci > 0.0 && abs > 0.0 {
                let a = anchor_scale();
                ColourScale::clamped(a.floor * per_ci / abs, a.span_decades)
            } else {
                peak_scale(field_peak)
            }
        }
        MapBasis::ChiOverQ => peak_scale(field_peak),
        MapBasis::DoseRate => dose_rate_anchor_scale(),
    }
}

/// What a texture was built from. A **change detector**, not a content key --
/// see [`MapTabState::built_for`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct TextureKey {
    cells: usize,
    plume_time_s: f64,
    basis: MapBasis,
    weighting: FieldWeighting,
    scale: ColourScale,
}

/// The Map tab's retained state.
///
/// Only the field texture and the key it was built for. Everything else the
/// tab draws is rebuilt per frame from the snapshot, which is the crate's
/// usual arrangement; a texture is the exception because it is a GPU upload,
/// and re-uploading a 512 x 512 image every frame would cost more than
/// computing the field does.
#[derive(Default)]
pub struct MapTabState {
    /// The uploaded field, one texel per grid cell.
    texture: Option<TextureHandle>,
    /// What the texture was built from.
    ///
    /// ~~"The field changes only when one of these does -- see
    /// `DispersionGrid` -- so this is the complete upload key and not an
    /// approximation of one."~~ **CORRECTED 2026-09-27.** With a marched puff
    /// population (gh:#344) the field depends on the whole *history* of the
    /// wind, which no fixed-size key can carry; the physics side says exactly
    /// this about its own `FieldKey`. This is a **change detector**, not a
    /// content key. It is sufficient for that, and only for that, because the
    /// population's clock advances whenever the population does -- so a moved
    /// plume always arrives with a new clock. Do not reuse a texture across a
    /// key match as though the key determined the field. Since 2026-09-28 it
    /// also carries the basis, the release-rate factor and the colour scale,
    /// because each of those changes the painted colours without moving the
    /// clock.
    built_for: Option<TextureKey>,
    /// The basis the operator asked for (default Absolute).
    basis: MapBasis,
    /// Operator-set colour scale per basis, indexed by [`MapBasis::index`].
    /// `None` until a slider is touched: the default then tracks
    /// [`default_scale`], which for Per Ci moves with the release rates.
    scales: [Option<ColourScale>; 4],
}

impl MapTabState {
    /// The scale in force for `basis`.
    fn scale_for(&self, basis: MapBasis, s: &HtgrSnapshot, field_peak: f64) -> ColourScale {
        self.scales[basis.index()].unwrap_or_else(|| default_scale(basis, s, field_peak))
    }

    /// The texture for this snapshot's field, re-uploading only when the field
    /// or the way it is painted has changed.
    ///
    /// Returns `None` when there is no field yet, which is the state before
    /// the first dispersion evaluation. Nothing is substituted in that case:
    /// an invented plume would look exactly like a computed one.
    fn field_texture(
        &mut self,
        ui: &Ui,
        s: &HtgrSnapshot,
        basis: MapBasis,
        scale: ColourScale,
    ) -> Option<&TextureHandle> {
        let cells = s.dispersion_grid_cells;
        if cells == 0 || s.dispersion_grid.len() < cells * cells {
            return None;
        }
        let key = TextureKey {
            cells,
            plume_time_s: s.dispersion_grid_time_s,
            basis,
            weighting: s.dispersion_grid_weighting,
            scale,
        };
        if self.built_for != Some(key) || self.texture.is_none() {
            let pixels: Vec<Color32> = s.dispersion_grid[..cells * cells]
                .iter()
                .map(|chi| scale.shade(field_value(*chi as f64, basis, s)))
                .collect();
            let image = ColorImage::new([cells, cells], pixels);
            match &mut self.texture {
                // NEAREST, not LINEAR: at one texel per screen pixel there is
                // nothing to interpolate, and filtering would blur evaluated
                // values into each other -- which is exactly the "picture of a
                // plume rather than a readout of one" the rose's docs reject.
                Some(handle) => handle.set(image, TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        "htgr_dispersion_field",
                        image,
                        TextureOptions::NEAREST,
                    ))
                }
            }
            self.built_for = Some(key);
        }
        self.texture.as_ref()
    }
}

/// The largest sample in the snapshot's field, in the unit of
/// `dispersion_grid_weighting` (~~`chi/Q` \[s/m^3\]~~ before gh:#400).
fn field_peak_sample(s: &HtgrSnapshot) -> f64 {
    s.dispersion_grid.iter().copied().fold(0.0_f32, f32::max) as f64
}

/// The basis selector and the two colour-scale sliders.
///
/// Returns the basis actually painted and the scale in force.
fn draw_scale_controls(
    ui: &mut Ui,
    s: &HtgrSnapshot,
    state: &mut MapTabState,
) -> (MapBasis, ColourScale) {
    let peak_chi = field_value(field_peak_sample(s), MapBasis::ChiOverQ, s);
    ui.horizontal_wrapped(|ui| {
        ui.label("Map shows:");
        for basis in MapBasis::ALL {
            ui.radio_value(&mut state.basis, basis, basis.label());
        }
    });
    let basis = effective_basis(state.basis, s);
    if basis != state.basis {
        ui.colored_label(
            Color32::from_rgb(200, 120, 20),
            "Absolute source term unavailable (no published inventory for a tracked nuclide, \
             or no release evaluated yet) -- showing Per Ci instead.",
        );
    }

    let mut scale = state.scale_for(basis, s, peak_chi);
    let before = scale;
    let mut reset = false;
    let unit = basis.unit();
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Slider::new(
                &mut scale.floor,
                ColourScale::FLOOR_MIN..=ColourScale::FLOOR_MAX,
            )
            .logarithmic(true)
            .custom_formatter(|v, _| format!("{v:.3e}"))
            .text(format!("minimum reading [{unit}]")),
        )
        .on_hover_text(
            "Colour-scale floor: readings at or below it are drawn light grey (no colour).",
        );
        ui.add(
            egui::Slider::new(
                &mut scale.span_decades,
                ColourScale::SPAN_MIN..=ColourScale::SPAN_MAX,
            )
            .text("additional range on top [decades]"),
        )
        .on_hover_text("Top of the colour scale = minimum x 10^span; readings above it clamp.");
        if ui
            .button("Reset scale")
            .on_hover_text("Back to this basis's default scale.")
            .clicked()
        {
            state.scales[basis.index()] = None;
            scale = default_scale(basis, s, peak_chi);
            reset = true;
        }
    });
    // Stored only when a slider actually moved, so an untouched basis keeps
    // tracking its (possibly moving) default.
    let moved = scale != before;
    let scale = ColourScale::clamped(scale.floor, scale.span_decades);
    if moved && !reset {
        state.scales[basis.index()] = Some(scale);
    }
    ui.label(format!(
        "Colour scale {:.3e} -> {:.3e} {unit} (log, {:.2} decades).",
        scale.floor,
        scale.top(),
        scale.span_decades
    ));
    if basis == MapBasis::Absolute {
        ui.label(format!(
            "Default anchors: floor {:.3e} = \"≈ banana\";  top {:.3e} = \"≈ 1 Sv-equivalent \
             (indicative — not a dose calculation)\".  Both are Bq of INTAKE placed numerically \
             on this Bq/m^3 scale -- colour anchors, not a unit conversion, and no dose is \
             computed here.",
            banana_anchor(),
            one_sievert_anchor()
        ))
        .on_hover_text(
            "Derived from US EPA Federal Guidance Report No. 11 (EPA-520/1-88-020, 1988), \
             Table 2.2 (ingestion, 'Effective' column): K-40 5.02e-9 Sv/Bq and Cs-137 \
             1.35e-8 Sv/Bq. Arithmetic and provenance: htgr_sim_v1/reference/References.md.",
        );
    }
    if basis == MapBasis::DoseRate {
        ui.colored_label(Color32::from_rgb(200, 60, 20), DOSE_RATE_FRAMING);
        ui.label(format!(
            "Default anchors (maintainer's figures, indicative, not regulatory): floor {:.1} µSv/h \
             = \"≈ airliner at cruise altitude\";  top {:.0} µSv/h = 1 mSv/h \"high\".",
            FLOOR_USV_PER_H, TOP_USV_PER_H
        ))
        .on_hover_text(dose_rate_method_text());
        draw_dose_scale_ramp_with_reference(ui, scale);
        ui.label(dose_rate_missing_note());
        if !basis.factor(s).is_finite() {
            ui.colored_label(
                Color32::from_rgb(200, 120, 20),
                "Dose rate unavailable: no absolute release rate for every tracked nuclide yet \
                 (no release evaluated, or no published inventory). Nothing is painted.",
            );
        }
    }
    (basis, scale)
}

/// The dose-rate colour ramp as a strip, floor to top, with the
/// [`nrc_2023_epz_reference_usv_per_h`] **reference marker** on it (a tick and
/// its label, not a colour change): the colours are exactly the map's, from
/// [`ColourScale::shade`]. The marker is drawn only while 104.2 µSv/h is
/// inside the current scale; otherwise a line says where it is.
fn draw_dose_scale_ramp_with_reference(ui: &mut Ui, scale: ColourScale) {
    let width = ui.available_width().clamp(200.0, 520.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 18.0), Sense::hover());
    let rect = response.rect;
    let segments = 96;
    for i in 0..segments {
        let f0 = i as f64 / segments as f64;
        let value = scale.floor * 10f64.powf((f0 + 0.5 / segments as f64) * scale.span_decades);
        let x0 = rect.left() + rect.width() * f0 as f32;
        let x1 = rect.left() + rect.width() * ((i + 1) as f64 / segments as f64) as f32;
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom())),
            0.0,
            scale.shade(value),
        );
    }
    let reference = nrc_2023_epz_reference_usv_per_h();
    match scale.fraction(reference) {
        Some(f) if reference < scale.top() => {
            let x = rect.left() + rect.width() * f as f32;
            painter.line_segment(
                [
                    egui::pos2(x, rect.top() - 2.0),
                    egui::pos2(x, rect.bottom() + 2.0),
                ],
                egui::Stroke::new(2.0, Color32::BLACK),
            );
            ui.small(format!(
                "Tick at {reference:.1} µSv/h: {NRC_2023_EPZ_REFERENCE_LABEL}. Constant-rate \
                 average; a front-loaded release reaches 10 mSv sooner."
            ));
        }
        _ => {
            ui.small(format!(
                "{reference:.1} µSv/h ({NRC_2023_EPZ_REFERENCE_LABEL}) is outside the current \
                 colour scale."
            ));
        }
    }
}

/// Draw the dispersion rose: the evaluated `chi/Q` field, with the receptor
/// ring, distance rings and sector spokes over it.
///
/// # Why a rose and not a contour plot
///
/// A contour plot of a Gaussian plume looks authoritative and would be, here,
/// an interpolation between 24 evaluated points -- a picture of a plume rather
/// than a readout of one, which this crate's rule forbids. The rose draws
/// exactly the points that were computed and nothing between them. It also
/// makes the sector structure visible, so nobody mistakes the resolution for
/// finer than it is.
///
/// The field underneath it is subject to the same rule and satisfies it the
/// other way: at one cell per screen pixel there is no gap left to
/// interpolate across, so nothing on it is drawn that was not evaluated.
///
/// Returns the side of the map square actually painted \[points\], which is
/// what the caller turns into the next frame's resolution request.
fn draw_dispersion_rose(
    ui: &mut Ui,
    s: &HtgrSnapshot,
    state: &mut MapTabState,
    side: f32,
    basis: MapBasis,
    scale: ColourScale,
) -> f32 {
    let (response, painter) = ui.allocate_painter(Vec2::new(side, side), Sense::hover());
    let rect = response.rect;
    let centre = rect.center();
    let size = rect.width().min(rect.height());
    let max_radius = size * 0.40;

    // White ground, not the dark canvas this used to have (maintainer,
    // 2026-09-24). A map is read against paper, and a dark field makes the
    // low-concentration sectors -- most of the plot -- the hardest part to
    // see, which is backwards: the quiet sectors are the reassuring result.
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_gray(180)),
        egui::StrokeKind::Inside,
    );

    let outermost = s
        .receptors
        .iter()
        .map(|r| r.distance_m)
        .fold(0.0_f64, f64::max);
    if outermost <= 0.0 {
        painter.text(
            centre,
            Align2::CENTER_CENTER,
            "no dispersion run yet",
            FontId::proportional(11.0),
            Color32::from_gray(120),
        );
        return rect.width();
    }

    // Distance rings, drawn at the real radii so the plot is to scale.
    let mut drawn: Vec<f64> = Vec::new();
    for receptor in &s.receptors {
        if receptor.distance_m > 0.0
            && !drawn.iter().any(|d| (d - receptor.distance_m).abs() < 1e-9)
        {
            drawn.push(receptor.distance_m);
        }
    }

    // --- the evaluated field, one texel per grid cell ---
    //
    // Painted FIRST so the rings, spokes and receptor markers sit on top of
    // it. Every cell is a real evaluation of the puff model at that cell's
    // coordinates, so this is a readout at every pixel of the square rather
    // than an interpolation between 24 -- which is what the rose's own docs
    // rule out.
    // The peak in the PAINTED unit, for the readout below.
    let field_peak = field_value(field_peak_sample(s), basis, s);
    let grid_px = max_radius * (s.dispersion_grid_half_width_m / outermost) as f32;
    if let Some(texture) = state.field_texture(ui, s, basis, scale) {
        let field_rect = Rect::from_center_size(centre, Vec2::splat(2.0 * grid_px));
        painter.image(
            texture.id(),
            field_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    drawn.sort_by(|a, b| a.partial_cmp(b).expect("finite distances"));
    for distance in &drawn {
        let r = max_radius * (*distance / outermost) as f32;
        painter.circle_stroke(centre, r, Stroke::new(1.0, Color32::from_black_alpha(90)));
        // Label each ring on its own circle. The rings ARE the distance
        // scale, so naming them is what turns the plot from a decoration
        // into something a reader can take a number off.
        painter.text(
            Pos2::new(centre.x + r, centre.y - 5.0),
            Align2::LEFT_BOTTOM,
            format!("{distance:.0} m"),
            FontId::proportional(9.0),
            Color32::from_gray(110),
        );
    }
    // Sector spokes, so the 45-degree resolution is visible rather than
    // implied by the marker spacing.
    for sector in 0..8 {
        let (sx, sy) = bearing_to_plot(45.0 * sector as f64, 1.0);
        painter.line_segment(
            [
                centre,
                Pos2::new(
                    centre.x + max_radius * sx as f32,
                    centre.y - max_radius * sy as f32,
                ),
            ],
            Stroke::new(0.5, Color32::from_black_alpha(40)),
        );
    }

    // ~~The peak sets the shading scale [...] logarithmic over four decades
    // below the peak~~ CHANGED 2026-09-28 (maintainer direction): the scale is
    // still logarithmic -- the field spans tens of decades, so a linear one
    // would show only the centreline -- but its floor and span are now the
    // operator's two sliders (`draw_scale_controls`), defaulting to the cited
    // anchors on the absolute basis. Four-decades-below-peak survives as the
    // chi/Q basis's default. Stated on screen below.
    // ~~24 shaded receptor discs, one per (bearing, distance) pair~~
    // **REMOVED 2026-09-27**, maintainer direction: *"receptor ring doesn't need
    // to be there now, just want a ring showing distances and a central arrow
    // showing wind direction."*
    //
    // They were a 24-point sampling of a quantity the field now renders at every
    // pixel, drawn ON TOP of that field -- so they occluded the very thing they
    // were a coarse summary of, and their log-shaded fill invited being read as
    // a second, disagreeing picture of the same plume. Worse, they shaded on the
    // TIME-INTEGRATED `chi_over_q` while the field underneath is instantaneous:
    // two different quantities in one image, distinguishable only by shape.
    //
    // The sampled numbers are NOT gone -- the maintainer asked for them live, and
    // they are in the table below, now including an INSTANTANEOUS column that
    // does agree with the field cell under it. What is gone is drawing them over
    // the map. ~~`log_shade` survives because the field still uses it.~~
    // (`log_shade` was replaced by `ColourScale::shade` on 2026-09-28.)

    // The wind arrow, drawn CENTRALLY and pointing the way the plume TRAVELS --
    // the opposite of the meteorological "from" bearing the operator dials in.
    // Derived from the snapshot, never from the layout.
    //
    // Now the only overlay besides the distance rings, so it carries the whole
    // "which way is the wind going" job and is drawn to be read at a glance:
    // a shaft from the stack, a filled head, and the bearing in words.
    let travel_deg = s.wind_from_deg + 180.0;
    let (wx, wy) = bearing_to_plot(travel_deg, 1.0);
    let tip = Pos2::new(
        centre.x + max_radius * 1.08 * wx as f32,
        centre.y - max_radius * 1.08 * wy as f32,
    );
    let wind_colour = Color32::from_rgb(20, 90, 190);
    painter.line_segment([centre, tip], Stroke::new(3.0, wind_colour));

    // Arrowhead: two short segments back down the shaft, rotated +/- 25 deg.
    // Built from the arrow's OWN direction rather than from screen axes, so it
    // stays correct at every bearing.
    let (dx, dy) = (tip.x - centre.x, tip.y - centre.y);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    let head = (max_radius * 0.12).max(6.0);
    for sign in [-1.0_f32, 1.0] {
        let a = sign * 25.0_f32.to_radians();
        let (ca, sa) = (a.cos(), a.sin());
        // Rotate the REVERSED unit vector, so the barbs trail behind the tip.
        let (bx, by) = (-ux * ca - -uy * sa, -ux * sa + -uy * ca);
        painter.line_segment(
            [tip, Pos2::new(tip.x + head * bx, tip.y + head * by)],
            Stroke::new(3.0, wind_colour),
        );
    }
    // The stack itself, so the arrow visibly starts AT the release point.
    painter.circle_filled(centre, 3.0, wind_colour);
    painter.text(
        Pos2::new(tip.x, tip.y + 10.0),
        Align2::CENTER_CENTER,
        format!("plume -> {travel_deg:.0} deg"),
        FontId::proportional(9.0),
        wind_colour,
    );

    painter.text(
        Pos2::new(centre.x, rect.top() + 8.0),
        Align2::CENTER_CENTER,
        "N",
        FontId::proportional(11.0),
        Color32::from_gray(60),
    );
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        format!("outer ring {outermost:.0} m"),
        FontId::proportional(9.0),
        Color32::from_gray(110),
    );
    // The colour scale, in the one place it cannot be mistaken for anything
    // else: what the brightest pixel on screen is worth, and how far down the
    // ramp runs. Without it the picture is a shape with no magnitude, and the
    // magnitude is the only thing anyone should quote off it.
    // A field wholly under the floor paints nothing, and a blank map must not
    // be read as "no plume". Measured 2026-09-28 (`tests::the_chi_over_q_table_is_the_maps_dilution_factor`):
    // at a 1200 K kernel the absolute peak is ~1.7e-4 Bq/m^3, about five
    // decades under the default "≈ banana" floor -- so on the default scale a
    // normal-operation plume is ENTIRELY below the floor. Said on the map.
    if let Some(note) = below_floor_note(field_peak, scale, basis) {
        painter.text(
            Pos2::new(centre.x, rect.top() + 24.0),
            Align2::CENTER_TOP,
            note,
            FontId::proportional(10.0),
            Color32::from_rgb(200, 120, 20),
        );
    }
    if field_peak.is_finite() {
        painter.text(
            Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0),
            Align2::RIGHT_BOTTOM,
            format!(
                "field peak {field_peak:.3e} {}; colour {:.2e}..{:.2e} ({} x {} cells)",
                basis.unit(),
                scale.floor,
                scale.top(),
                s.dispersion_grid_cells,
                s.dispersion_grid_cells
            ),
            FontId::proportional(9.0),
            Color32::from_gray(110),
        );
    }

    rect.width()
}

/// The on-map note for a field whose peak is at or below the colour floor,
/// or `None` when some of it is coloured.
fn below_floor_note(field_peak: f64, scale: ColourScale, basis: MapBasis) -> Option<String> {
    if !field_peak.is_finite() || field_peak > scale.floor {
        return None;
    }
    // Why the field is so low, on the basis where it matters most: the
    // dose-rate anchors are everyday exposure levels, and a normal-operation
    // leak sits many decades under them. Said, so a grey map is not read as
    // a broken one.
    let why = if basis == MapBasis::DoseRate {
        " The normal-operation release is tiny (five nuclides leaking from the primary \
         circuit at ~1 %/day), so its indicative dose rate is far below 0.8 µSv/h."
    } else {
        ""
    };
    Some(if field_peak > 0.0 {
        format!(
            "Whole field below the minimum reading: peak {field_peak:.2e} {} is {:.1} decades \
             under the floor.{why} Lower \"minimum reading\" to see the plume.",
            basis.unit(),
            (scale.floor / field_peak).log10()
        )
    } else {
        "Field is zero on this basis (no release evaluated, or zero release rate). \
         The chi/Q basis shows the plume shape."
            .to_string()
    })
}

/// Unit-circle position for a compass bearing, `(x east, y north)`.
///
/// Same convention as
/// [`crate::physics::atmospheric_dispersion::bearing_to_site_frame`]: sine on
/// east, cosine on north, so bearing 0 is north.
fn bearing_to_plot(bearing_deg: f64, radius: f64) -> (f64, f64) {
    let radians = bearing_deg.to_radians();
    (radius * radians.sin(), radius * radians.cos())
}

// ~~`log_shade(value, peak)`~~ -- REPLACED 2026-09-28 by `ColourScale::shade`,
// which takes the floor and span from the operator's sliders instead of fixing
// them at four decades below the peak. The peak-relative convention survives as
// `peak_scale`, the chi/Q basis's default. One behavioural difference, on
// purpose: a value below the floor is now the light "no reading" grey rather
// than the bottom ramp colour, because the floor is a *minimum reading*.

/// Smallest angle between two compass bearings, degrees.
fn angular_distance(a_deg: f64, b_deg: f64) -> f64 {
    let diff = (a_deg - b_deg).rem_euclid(360.0);
    diff.min(360.0 - diff)
}

/// Format a duration in seconds as `h:mm:ss`, for the two clocks.
fn clock_text(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "--".to_string();
    }
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// The plume clock readout and its fast-forward buttons.
///
/// # What these buttons move, and what they deliberately do not
///
/// They move the **plume** clock. The plant clock beside them is untouched,
/// and both are on screen at once so the difference cannot be missed.
///
/// That split is not a convenience: the plant model cannot skip time (its
/// timestep is pinned by an advective Courant limit -- see
/// [`crate::physics::PLANT_TIMESTEP_S`]) and it computes at roughly real time,
/// so a genuine one-hour plant jump costs an hour. The dispersion field has no
/// such constraint, because `chi/Q` is a dilution factor that does not depend
/// on the source at all, so the plume clock can be moved without the plant
/// having to compute the interval it covers. Maintainer direction, 2026-09-25,
/// chose this split knowing it.
///
/// # ~~"exact rather than extrapolated"~~ **CORRECTED 2026-09-27**
///
/// ~~"jumping its clock is the same closed form evaluated at a later argument,
/// exact rather than extrapolated
/// (`atmospheric_dispersion::tests::a_plume_clock_jump_equals_having_run_the_clock_there`
/// pins that)"~~. That claim is what made an unqualified fast-forward look
/// safe, and it is **false** since gh:#344 made the field a marched Lagrangian
/// puff population (`AtmosphericDispersionChannel::advance_population`): a
/// model with history cannot be evaluated at an arbitrary clock without
/// running the history. A forward jump is **marched under the CURRENT wind**,
/// so it is an extrapolation under "the wind held constant over the jumped
/// interval", not a prediction across a wind change. The test that survives is
/// `a_plume_clock_jump_equals_having_run_the_clock_there_on_a_steady_wind`
/// (same module) -- the `_on_a_steady_wind` qualifier it gained is the whole
/// content of the correction.
///
/// A **rewind** is not merely approximate. The trajectory integral is not
/// invertible and no per-step history is kept, so the population is cleared
/// and re-marched from the stack under the current wind: the plume restarts,
/// and any bend it carried from an earlier wind change is gone. Both facts are
/// on screen at the buttons that cause them, per this crate's rule that a
/// limitation belongs where the reader meets the result.
fn draw_plume_clock(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, s: &HtgrSnapshot) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Plant clock {}", clock_text(s.sim_time_s)));
        ui.separator();
        ui.label(format!(
            "Plume clock {}",
            clock_text(s.dispersion_grid_time_s)
        ));
        ui.separator();
        ui.label("Fast forward the plume:");
        for (jump_s, label) in PLUME_JUMPS_S {
            // Forward and backward are different operations on a marched
            // population, so they get different hover text. See
            // `PLUME_JUMPS_S` and this function's doc comment.
            let hover = if jump_s >= 0.0 {
                "Moves the PLUME clock only. The reactor, the release channel and the table \
                 below stay on the plant clock -- those depend on the source and the plant \
                 cannot skip time. The plume is a MARCHED puff population, so the jump is \
                 marched under the CURRENT wind: read it as 'the plume this wind would build \
                 if it held that long', NOT as a forecast across a wind change."
            } else {
                "Rewinds the PLUME clock only -- and CLEARS the puff population. The march \
                 cannot be undone (the trajectory integral is not invertible and no per-step \
                 history is kept), so the plume restarts from the stack under the current \
                 wind and any bend from an earlier wind change is lost."
            };
            if ui.button(label).on_hover_text(hover).clicked() {
                let offset = (s.plume_clock_offset_s + jump_s).max(-s.sim_time_s);
                physics.update(|state| state.plume_clock_offset_s = offset);
            }
        }
        if ui
            .button("Now")
            .on_hover_text(
                "Puts the plume clock back on the plant clock. From a fast-forward that is a \
                 REWIND, so it CLEARS the puff population: the plume restarts from the stack \
                 under the current wind.",
            )
            .clicked()
        {
            physics.update(|state| state.plume_clock_offset_s = 0.0);
        }
    });
    if let Some(banner) = plume_offset_banner(s.plume_clock_offset_s) {
        ui.colored_label(Color32::from_rgb(200, 120, 20), banner);
    }
}

/// The warning banner for a plume clock that is not on the plant clock, or
/// `None` when the two agree.
///
/// # Why this is a function and not inline in the layout
///
/// It is the only place the operator is told what a displaced plume clock
/// means, and since gh:#344 the meaning **differs by sign** -- a forward offset
/// is an extrapolation under the current wind, a backward one is a cleared and
/// restarted population. A banner built inline in an `egui` closure cannot be
/// asserted by a test; pulled out, it can, and
/// `tests::the_offset_banner_says_extrapolation_forwards_and_restart_backwards`
/// pins both branches.
///
/// ~~"Plume clock is running … AHEAD of the plant"~~ **CORRECTED 2026-09-27**:
/// the old banner was emitted for `abs(offset) > 0` and said AHEAD either way,
/// so a rewind -- the case with the *larger* caveat -- was described as a
/// fast-forward.
fn plume_offset_banner(offset_s: f64) -> Option<String> {
    if !(offset_s.abs() > f64::EPSILON) {
        return None;
    }
    let magnitude = clock_text(offset_s.abs());
    Some(if offset_s > 0.0 {
        format!(
            "Plume clock is running {magnitude} AHEAD of the plant. The map is an \
             EXTRAPOLATION: the puff population was marched under the CURRENT wind, so it is \
             the plume this wind would build if it held that long -- not a forecast across a \
             wind change. The plant state, the release and the table below are still at the \
             plant clock. The plume settles after one puff lifetime (20 min), so past that a \
             further jump changes nothing unless the wind does."
        )
    } else {
        format!(
            "Plume clock is {magnitude} BEHIND the plant. Rewinding CLEARED the puff \
             population -- the march is not invertible and no per-step history is kept -- so \
             the plume has RESTARTED from the stack under the current wind, with any earlier \
             wind change forgotten. The plant state, the release and the table below are \
             still at the plant clock."
        )
    })
}

/// The downwind half of the receptor ring, sorted by distance then bearing.
///
/// Only the downwind half is tabulated: 24 rows is a wall, and the upwind
/// receptors are 20+ orders below the centreline and carry no information a
/// reader acts on.
///
/// ~~"The ROSE shows all 24, so nothing is hidden."~~ **CORRECTED 2026-09-27**
/// -- the rose no longer plots receptors at all (maintainer direction; it shows
/// the field, the distance rings and the wind arrow). So the upwind half is now
/// genuinely not on screen in the two activity tables. That is a table-length
/// choice and not a filter on what was COMPUTED -- all 24 are evaluated, and
/// since 2026-09-28 the `chi/Q` table below them shows all eight bearings.
fn downwind_rows(s: &HtgrSnapshot) -> Vec<&super::state::ReceptorSnapshot> {
    let travel_deg = (s.wind_from_deg + 180.0).rem_euclid(360.0);
    let mut rows: Vec<&super::state::ReceptorSnapshot> = s
        .receptors
        .iter()
        .filter(|r| r.distance_m > 0.0 && angular_distance(r.bearing_deg, travel_deg) <= 90.0)
        .collect();
    rows.sort_by(|a, b| {
        a.distance_m
            .total_cmp(&b.distance_m)
            .then(a.bearing_deg.total_cmp(&b.bearing_deg))
    });
    rows
}

/// Column headings of the per-Ci table. **Unchanged** since 2026-09-27
/// (maintainer direction 2026-09-28: keep the per-Ci tables as they are);
/// pinned by `tests::the_per_ci_table_is_unchanged`.
const PER_CI_TABLE_HEADINGS: [&str; 6] = [
    "Bearing",
    "Distance",
    "chi/Q LIVE [s/m^3]",
    "chi/Q integrated [s/m^3]",
    "Air [Bq.s/m^3 per Ci]",
    "Ground [Bq/m^2 per Ci]",
];

/// Column headings of the absolute table, every quantity with its true unit.
const ABSOLUTE_TABLE_HEADINGS: [&str; 5] = [
    "Bearing",
    "Distance",
    "Air LIVE [Bq/m^3]",
    "Air integrated [Bq.s/m^3]",
    "Ground, dry [Bq/m^2]",
];

/// One distance's row pair of the `chi/Q` table: the eight bearings, north
/// first, clockwise, with the LIVE (instantaneous) and integrated `chi/Q`.
#[derive(Debug, Clone, PartialEq)]
struct ChiOverQRow {
    distance_m: f64,
    bearings_deg: Vec<f64>,
    /// Instantaneous `chi/Q` \[s/m^3\] -- the dilution factor the MAP uses
    /// (same kernel, same puff population; see
    /// `atmospheric_dispersion::tests::the_live_ring_sample_agrees_with_the_field_cell_under_it`).
    live: Vec<f64>,
    /// Time-integrated `chi/Q` \[s/m^3\] -- the dilution factor the ACTIVITY
    /// columns are built on.
    integrated: Vec<f64>,
}

/// The `chi/Q` table, one [`ChiOverQRow`] per receptor distance, all bearings.
/// Pure, so a test can check it against the physics.
fn chi_over_q_rows(s: &HtgrSnapshot) -> Vec<ChiOverQRow> {
    let mut distances: Vec<f64> = Vec::new();
    for r in &s.receptors {
        if r.distance_m > 0.0 && !distances.iter().any(|d| (d - r.distance_m).abs() < 1e-9) {
            distances.push(r.distance_m);
        }
    }
    distances.sort_by(f64::total_cmp);
    distances
        .into_iter()
        .map(|d| {
            let mut at: Vec<&super::state::ReceptorSnapshot> = s
                .receptors
                .iter()
                .filter(|r| (r.distance_m - d).abs() < 1e-9)
                .collect();
            at.sort_by(|a, b| a.bearing_deg.total_cmp(&b.bearing_deg));
            ChiOverQRow {
                distance_m: d,
                bearings_deg: at.iter().map(|r| r.bearing_deg).collect(),
                live: at.iter().map(|r| r.instantaneous_chi_over_q).collect(),
                integrated: at.iter().map(|r| r.chi_over_q).collect(),
            }
        })
        .collect()
}

/// A number, or `--` when it is not available.
fn sci_or_dash(value: f64, digits: usize) -> String {
    if value.is_finite() {
        format!("{value:.digits$e}")
    } else {
        "--".to_string()
    }
}

/// How the dose rate is computed, for hover text.
fn dose_rate_method_text() -> &'static str {
    "Computed by buangkok (pathway functions shared with its pyDOSEIA port): cloud submersion \
     = live air concentration x US EPA FGR-15 (2025, EPA 402-R-25-001) Table 4-6 air-submersion \
     coefficient (semi-infinite cloud); committed inhalation = concentration x adult breathing \
     rate (8400 m^3/y) x FGR-11 Table 2.1 'Effective' coefficient (max over lung classes); \
     ground shine = dry deposit x FGR-15 Table 4-1. Adult. Cs-137 includes Ba-137m (0.944). \
     Summed over Kr-85, Xe-133, I-131, Cs-137, Ag-110m. The map pixel is submersion + \
     inhalation; ground shine is in the table only (no per-pixel deposit). Semi-infinite cloud: \
     over-states on a narrow plume's centreline, under-states under the elevated plume before \
     it grounds. Provenance: crates/buangkok/docs/References.md."
}

/// The on-screen list of (nuclide, pathway) terms with no coefficient.
fn dose_rate_missing_note() -> String {
    let missing = dose_rate::coefficients().missing();
    if missing.is_empty() {
        return "Every nuclide has a coefficient on every pathway.".to_string();
    }
    let list: Vec<String> = missing
        .iter()
        .map(|(n, p)| format!("{n} {}", p.label()))
        .collect();
    format!(
        "Missing coefficient (left OUT of the sums, not counted as zero): {}. FGR-11 publishes no \
         inhalation coefficient for noble gases; their inhalation dose is negligible next to \
         submersion, not zero.",
        list.join(", ")
    )
}

/// Column headings of the per-pathway split table.
const DOSE_SPLIT_HEADINGS: [&str; 4] = [
    "Nuclide",
    "Cloud submersion [µSv/h]",
    "Inhalation, committed [µSv/h]",
    "Ground shine [µSv/h]",
];

/// One distance's row pair of the dose-rate table: the eight bearings, north
/// first, clockwise.
#[derive(Debug, Clone, PartialEq)]
struct DoseRateRow {
    distance_m: f64,
    bearings_deg: Vec<f64>,
    /// Air pathways (submersion + committed inhalation) \[µSv/h\] -- the
    /// value of the dose-rate map pixel under the receptor
    /// ([`receptor_air_dose_rate`], the same emission-weighted sum).
    air: Vec<f64>,
    /// Ground shine from the dry deposit \[µSv/h\]; `NAN` when unavailable.
    /// Not on the map.
    ground: Vec<f64>,
}

/// Ground-shine rate at one receptor \[µSv/h\], summed over the nuclides
/// through `buangkok` (every tracked nuclide has a ground coefficient, so the
/// sum is complete); `NAN` when the deposit is unavailable.
fn receptor_ground_rate(r: &super::state::ReceptorSnapshot) -> f64 {
    let split = dose_rate::receptor_split(
        1.0,
        &r.instantaneous_air_bq_per_m3_by_nuclide,
        &r.ground_bq_per_m2_absolute_by_nuclide,
        dose_rate::coefficients(),
    );
    let (total, missing) = dose_rate::pathway_total(&split, Pathway::GroundShine);
    if missing.is_empty() {
        total
    } else {
        f64::NAN
    }
}

/// The dose-rate table, one [`DoseRateRow`] per receptor distance, all
/// bearings. Pure, so a test can check it against the map pixel.
fn dose_rate_rows(s: &HtgrSnapshot) -> Vec<DoseRateRow> {
    chi_over_q_rows(s)
        .into_iter()
        .map(|row| {
            let at: Vec<&super::state::ReceptorSnapshot> = row
                .bearings_deg
                .iter()
                .map(|b| {
                    s.receptors
                        .iter()
                        .find(|r| {
                            (r.distance_m - row.distance_m).abs() < 1e-9
                                && (r.bearing_deg - b).abs() < 1e-9
                        })
                        .expect("receptor from the same ring")
                })
                .collect();
            DoseRateRow {
                distance_m: row.distance_m,
                bearings_deg: row.bearings_deg.clone(),
                air: at.iter().map(|r| receptor_air_dose_rate(r)).collect(),
                ground: at.iter().map(|r| receptor_ground_rate(r)).collect(),
            }
        })
        .collect()
}

/// A dose-rate cell: the number, marked when at or below the colour floor,
/// `unavailable` when there is none. **Never printed as 0 for a missing
/// value**, and a below-floor value keeps its number.
fn dose_cell_text(value: f64, floor: f64) -> String {
    if !value.is_finite() {
        "unavailable".to_string()
    } else if value <= floor {
        format!("{value:.2e} (below floor)")
    } else {
        format!("{value:.3e}")
    }
}

/// [`dose_cell_text`] with a `*` appended above the
/// [`nrc_2023_epz_reference_usv_per_h`] reference figure (see the table's
/// caption for what the flag means and does not mean).
fn dose_cell_text_with_reference(value: f64, floor: f64) -> String {
    let text = dose_cell_text(value, floor);
    if value.is_finite() && value > nrc_2023_epz_reference_usv_per_h() {
        format!("{text} *")
    } else {
        text
    }
}

/// The receptor with the highest air-pathway dose rate, for the split table.
fn peak_air_receptor(s: &HtgrSnapshot) -> Option<&super::state::ReceptorSnapshot> {
    s.receptors
        .iter()
        .filter(|r| r.distance_m > 0.0)
        .max_by(|a, b| receptor_air_dose_rate(a).total_cmp(&receptor_air_dose_rate(b)))
}

/// The whole-table note when every air cell is below the floor, or `None`.
fn dose_table_below_floor_note(rows: &[DoseRateRow], floor: f64) -> Option<String> {
    let peak = rows
        .iter()
        .flat_map(|r| r.air.iter())
        .copied()
        .filter(|v| v.is_finite())
        .fold(f64::NAN, f64::max);
    below_floor_note(peak, ColourScale::clamped(floor, 1.0), MapBasis::DoseRate)
}

/// The dose-rate tables (maintainer direction 2026-09-29: first, above the
/// absolute basis): by distance x bearing, then the per-pathway split at the
/// peak receptor.
fn draw_dose_rate_tables(ui: &mut Ui, s: &HtgrSnapshot, dose_scale: ColourScale) {
    ui.strong("Dose rate [µSv/h] by receptor distance -- INDICATIVE");
    ui.colored_label(Color32::from_rgb(200, 60, 20), DOSE_RATE_FRAMING);
    let floor = dose_scale.floor;
    let rows = dose_rate_rows(s);
    let bearings: Vec<f64> = rows
        .first()
        .map(|r| r.bearings_deg.clone())
        .unwrap_or_default();
    egui::Grid::new("htgr_map_dose_rate_by_distance")
        .num_columns(2 + bearings.len())
        .striped(true)
        .show(ui, |ui| {
            ui.label("Distance");
            ui.label("Pathways");
            for b in &bearings {
                ui.label(format!("{b:.0} deg"));
            }
            ui.end_row();
            for row in &rows {
                for (what, values) in [
                    ("air LIVE (= map pixel)", &row.air),
                    ("ground shine", &row.ground),
                ] {
                    ui.label(format!("{:.0} m", row.distance_m));
                    ui.label(what);
                    for v in values.iter() {
                        ui.label(dose_cell_text_with_reference(*v, floor))
                            .on_hover_text(dose_rate_method_text());
                    }
                    ui.end_row();
                }
            }
        });
    if let Some(note) = dose_table_below_floor_note(&rows, floor) {
        ui.colored_label(Color32::from_rgb(200, 120, 20), note);
    }
    ui.label(format!(
        "air LIVE = cloud submersion + committed inhalation from the live air concentration, the \
         same number as the dose-rate map pixel under the receptor. Ground shine = the dry \
         deposit one 1200 s puff run leaves x FGR-15 ground coefficient; not on the map (no \
         per-pixel deposit), and it under-states a release held longer. \"below floor\" = at or \
         under the {floor:.2} µSv/h colour floor. \"*\" = above {:.1} µSv/h, the {}; a \
         comparison figure for an indicative rate, not a zone boundary. {}",
        nrc_2023_epz_reference_usv_per_h(),
        NRC_2023_EPZ_REFERENCE_LABEL,
        dose_rate_missing_note()
    ));
    ui.add_space(4.0);

    let Some(peak) = peak_air_receptor(s) else {
        return;
    };
    ui.strong(format!(
        "Per-pathway split at the peak receptor ({:.0} deg, {:.0} m) [µSv/h]",
        peak.bearing_deg, peak.distance_m
    ));
    // Unit chi/Q against the receptor's per-nuclide live air concentration:
    // the emission-weighted air, not today's rate x instantaneous chi/Q.
    let split = dose_rate::receptor_split(
        1.0,
        &peak.instantaneous_air_bq_per_m3_by_nuclide,
        &peak.ground_bq_per_m2_absolute_by_nuclide,
        dose_rate::coefficients(),
    );
    egui::Grid::new("htgr_map_dose_rate_split")
        .num_columns(DOSE_SPLIT_HEADINGS.len())
        .striped(true)
        .show(ui, |ui| {
            for h in DOSE_SPLIT_HEADINGS {
                ui.label(h);
            }
            ui.end_row();
            for (k, name) in TRACKED_NUCLIDES.iter().enumerate() {
                ui.label(*name);
                for p in Pathway::ALL {
                    ui.label(match split[k][p.index()] {
                        None => "missing".to_string(),
                        Some(v) => sci_or_dash(v, 3),
                    });
                }
                ui.end_row();
            }
            ui.label("Total");
            for p in Pathway::ALL {
                let (total, missing) = dose_rate::pathway_total(&split, p);
                ui.label(if missing.is_empty() {
                    sci_or_dash(total, 3)
                } else {
                    format!("{} (excl. {})", sci_or_dash(total, 3), missing.join(", "))
                });
            }
            ui.end_row();
        });
    ui.add_space(6.0);
}

/// The dispersion tables and their caveats: dose rate (since 2026-09-29,
/// first), absolute, then per-Ci (unchanged), then `chi/Q` by distance
/// directly under them.
fn draw_dispersion_table(ui: &mut Ui, s: &HtgrSnapshot, dose_scale: ColourScale) {
    ui.label(format!(
        "Gaussian puff (changi::puff, ported from R `puff` 0.1.1) -- NOT FLEXPART.  \
         Wind {:.1} m/s FROM {:.0} deg, Pasquill class {}.  Last run at t = {}",
        s.wind_speed_m_per_s,
        s.wind_from_deg,
        if s.stability_class.is_empty() {
            "--"
        } else {
            s.stability_class
        },
        if s.dispersion_evaluated_at_s.is_finite() {
            format!("{:.0} s", s.dispersion_evaluated_at_s)
        } else {
            "--".to_string()
        }
    ));
    ui.label(format!(
        "Release rate to atmosphere (5 tracked nuclides, primary-circuit leak ~1 %/day): \
         absolute {} Bq/s;  per-Ci basis {} Bq/s per Ci.",
        sci_or_dash(s.dispersion_source_rate_absolute_bq_per_s, 3),
        sci_or_dash(s.dispersion_source_rate_per_ci_bq_per_s, 3),
    ));
    ui.label(
        "chi/Q is the quotable quantity: a dilution factor that does NOT depend on the source. \
         The activity columns DO: they are the five tracked nuclides only, leaked from the \
         primary circuit at the published ~1 %/day straight to the stack -- building not \
         credited (conservative; gh:#409): no retention, filtration or deposition -- with TRISO-ATOPS reference failure fractions -- NOT a source term and \
         not figures for any reactor. Research, education and V&V only. The dose-rate table \
         below is INDICATIVE (buangkok, US EPA coefficients) and is not a dose to anyone.",
    );
    ui.add_space(4.0);

    // --- 0. DOSE RATE (maintainer direction 2026-09-29: above the absolute
    // basis) ---
    draw_dose_rate_tables(ui, s, dose_scale);

    // --- 1. ABSOLUTE (maintainer direction 2026-09-28) ---
    ui.strong("Absolute basis");
    egui::Grid::new("htgr_map_dispersion_grid_absolute")
        .num_columns(ABSOLUTE_TABLE_HEADINGS.len())
        .striped(true)
        .show(ui, |ui| {
            for heading in ABSOLUTE_TABLE_HEADINGS {
                ui.label(heading);
            }
            ui.end_row();
            for r in downwind_rows(s) {
                ui.label(format!("{:.0} deg", r.bearing_deg));
                ui.label(format!("{:.0} m", r.distance_m));
                // The same emission-weighted sum the Absolute-basis texture
                // paints, so this equals the pixel under the receptor.
                ui.label(sci_or_dash(receptor_air_absolute(r), 3));
                ui.label(sci_or_dash(r.air_bq_s_per_m3_absolute, 3));
                ui.label(sci_or_dash(r.ground_bq_per_m2_absolute, 3));
                ui.end_row();
            }
        });
    ui.label(
        "Air LIVE = instantaneous chi/Q x absolute release rate: the instantaneous air \
         concentration, equal to the map pixel under the receptor on the Absolute basis \
         (decay in transit neglected, < 0.2 % over a 20 min puff life). Air integrated and \
         Ground are the time-integrated survey over the puff run, with decay in transit.",
    );
    ui.add_space(6.0);

    // --- 2. PER Ci -- unchanged ---
    ui.strong("Per curie of core inventory");
    egui::Grid::new("htgr_map_dispersion_grid")
        // 6, not 5: the LIVE chi/Q column was added 2026-09-27 beside the
        // time-integrated one. Both are shown because they are different
        // quantities, not two renderings of one.
        .num_columns(6)
        .striped(true)
        .show(ui, |ui| {
            for heading in PER_CI_TABLE_HEADINGS {
                ui.label(heading);
            }
            ui.end_row();
            for r in downwind_rows(s) {
                ui.label(format!("{:.0} deg", r.bearing_deg));
                ui.label(format!("{:.0} m", r.distance_m));
                // LIVE first, because it is the one that refreshes at 10 Hz and
                // the one that agrees with the map cell under the same point.
                ui.label(format!("{:.4e}", r.instantaneous_chi_over_q));
                ui.label(format!("{:.4e}", r.chi_over_q));
                ui.label(format!("{:.3e}", r.air_bq_s_per_m3));
                ui.label(format!("{:.3e}", r.ground_bq_per_m2));
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    ui.label(
        "Downwind half shown. LIVE chi/Q is the INSTANTANEOUS field sampled at each point and \
         refreshes with the map (10 Hz); on the chi/Q basis it is the same number as the map \
         cell under that point. Integrated chi/Q is the TIME-INTEGRATED dilution factor over \
         the whole puff run and refreshes on the 2 s dispersion throttle -- the activity \
         columns are built on THAT one. The two share units and will not agree. Ground \
         deposition is DRY only -- wet scavenging is not ported, so it is not an upper bound.",
    );
    ui.add_space(6.0);

    // --- 3. chi/Q by distance (maintainer direction 2026-09-28: directly
    // under the Bq/Ci tables) ---
    ui.strong("chi/Q by receptor distance [s/m^3] -- source-independent dilution factor");
    let rows = chi_over_q_rows(s);
    let bearings: Vec<f64> = rows
        .first()
        .map(|r| r.bearings_deg.clone())
        .unwrap_or_default();
    egui::Grid::new("htgr_map_chi_over_q_by_distance")
        .num_columns(2 + bearings.len())
        .striped(true)
        .show(ui, |ui| {
            ui.label("Distance");
            ui.label("chi/Q");
            for b in &bearings {
                ui.label(format!("{b:.0} deg"));
            }
            ui.end_row();
            for row in &rows {
                for (what, values) in [("LIVE", &row.live), ("integrated", &row.integrated)] {
                    ui.label(format!("{:.0} m", row.distance_m));
                    ui.label(what);
                    for v in values.iter() {
                        ui.label(format!("{v:.3e}"));
                    }
                    ui.end_row();
                }
            }
        });
    ui.label(
        "All eight bearings. LIVE is the instantaneous chi/Q the map multiplies by the release \
         rate; integrated is the time-integrated chi/Q the activity columns use. Neither \
         depends on any inventory, leak-rate or fuel input.",
    );
}

/// Cells per side to ask the dispersion field for, given the map square's side
/// in points and the display's points-to-pixels ratio.
///
/// **One evaluation per physical screen pixel** -- the whole point of the
/// 2026-09-25 direction. On a HiDPI display a point is more than a pixel, so
/// asking in points would still paint every cell across two or more pixels,
/// which is the "big box" being replaced, only smaller.
///
/// The physics clamps this to what the host can afford
/// ([`crate::physics::atmospheric_dispersion::max_grid_cells`]), so an
/// oversized request costs a coarser map, never a missed physics tick.
fn requested_cells(side_points: f32, pixels_per_point: f32) -> usize {
    (side_points * pixels_per_point).round().max(1.0) as usize
}

/// Whether a new resolution request is worth sending.
///
/// A window being dragged changes the map's width by a pixel at a time, and
/// every changed request invalidates the field cache and forces a fresh
/// evaluation. A 2 % dead band means a resize settles instead of recomputing
/// the field on every intermediate width, while any real size change still
/// gets through. Purely a rate-limiting choice; it cannot change a value.
fn resolution_request_changed(current: usize, wanted: usize) -> bool {
    let tolerance = (current as f64 * 0.02).max(2.0);
    (current as f64 - wanted as f64).abs() > tolerance
}

/// What the Map tab asks its host to do after a frame (gh:#400).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAction {
    /// Nothing this frame.
    None,
    /// **Reset plant**: discard the run and start the plant from its default
    /// operating point -- the same fresh start as the crash modal's restart
    /// (`HtgrSimApp::restart_simulation`), so there is one reset path.
    ResetPlant,
    /// Start the water-ingress accident (gh:#401).
    StartWaterIngress,
}

/// One accident-scenario button on the Map tab and whether it can be pressed
/// yet. A button stays disabled until the stage that implements its physics
/// lands (source-term plan, gh:#398): pressing a button whose release is not
/// modelled would draw an invented plume that looks exactly like a computed
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccidentButton {
    label: &'static str,
    enabled: bool,
    /// What pressing it asks of the host.
    action: MapAction,
    /// Why it is disabled, or what it does.
    hover: &'static str,
}

/// The accident buttons, in the order drawn.
const ACCIDENT_BUTTONS: [AccidentButton; 2] = [
    AccidentButton {
        label: "Water ingress",
        enabled: true,
        action: MapAction::StartWaterIngress,
        hover: "Two steam-generator tubes rupture and the secondary relief fails (Gao & Shi \
                2002 s.5.4): 129.9 kg of water, steam moderation, graphite-steam corrosion, \
                primary relief venting, SG wash-off and kernel hydrolysis (gh:#401). \
                Building not credited (gh:#409). Research and education only.",
    },
    AccidentButton {
        label: "DLOFC + ATWS + air ingress",
        enabled: false,
        action: MapAction::None,
        hover: "Not yet modelled: depressurised loss of forced cooling without scram, \
                with air ingress, lands with source-term stage 4 (gh:#402).",
    },
];

/// The scenario row: the accident buttons (disabled until their stage lands)
/// and **Reset plant**.
fn draw_scenario_buttons(ui: &mut Ui) -> MapAction {
    let mut action = MapAction::None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Scenarios:");
        for b in ACCIDENT_BUTTONS {
            if ui
                .add_enabled(b.enabled, egui::Button::new(b.label))
                .on_hover_text(b.hover)
                .on_disabled_hover_text(b.hover)
                .clicked()
            {
                action = b.action;
            }
        }
        if ui
            .button("Reset plant")
            .on_hover_text(
                "Discard this run and start the plant from its default operating point: \
                 power, temperatures, pools, building, plume and clock all reset. Map \
                 display settings are kept.",
            )
            .clicked()
        {
            action = MapAction::ResetPlant;
        }
    });
    action
}

/// Draw the whole Map tab: the Gaussian puff dispersion widgets.
///
/// `view` is the tab's viewport, measured by the caller **outside** the scroll
/// area -- inside one, the available height is the content's own budget rather
/// than the window's, so a panel that sized itself from in there would grow
/// every frame it filled.
///
/// Layout is the map square above the table, not beside it (maintainer,
/// 2026-09-25), with the map at [`MAP_HEIGHT_FRACTION`] of the viewport
/// height. The combination overflows the viewport by construction, which is
/// what the caller's two-way scroll area is for.
pub fn draw_map(
    ui: &mut Ui,
    physics: &SharedState<HtgrSnapshot>,
    s: &HtgrSnapshot,
    state: &mut MapTabState,
    view: Vec2,
) -> MapAction {
    ui.heading("Atmospheric dispersion -- Gaussian puff");
    let action = draw_scenario_buttons(ui);
    if s.water_ingress_triggered {
        ui.colored_label(
            Color32::from_rgb(200, 120, 20),
            format!(
                "WATER INGRESS running: primary {:.3} MPa, steam {:.1} kg, graphite gasified \
                 {:.2} kg, H2 {:.2} %, CO {:.2} %, primary gas vented {:.3} %. Building not \
                 credited (gh:#409). Research and education only.",
                s.ingress_pressure_mpa,
                s.ingress_steam_kg,
                s.ingress_graphite_corroded_kg,
                s.ingress_h2_percent,
                s.ingress_co_percent,
                100.0 * s.ingress_vented_fraction
            ),
        );
    }
    // The one puff-model configuration every basis and table below uses
    // (maintainer direction 2026-09-29; `map_puff_model`, gh:#384).
    ui.label(crate::physics::map_puff_model::regime_label());
    draw_plume_clock(ui, physics, s);
    ui.add_space(4.0);

    let (basis, scale) = draw_scale_controls(ui, s, state);
    ui.add_space(4.0);

    let side = (view.y * MAP_HEIGHT_FRACTION).max(MAP_MIN_SIDE);
    let painted = draw_dispersion_rose(ui, s, state, side, basis, scale);

    // Tell the physics what resolution this map can show. A control input,
    // written the same way the wind is -- see `HtgrSnapshot::
    // map_field_cells_requested`.
    let wanted = requested_cells(painted, ui.ctx().pixels_per_point());
    if resolution_request_changed(s.map_field_cells_requested, wanted) {
        physics.update(|state| state.map_field_cells_requested = wanted);
    }
    // And which basis to sum the field in (gh:#400), the same way.
    let weighting = basis.weighting();
    if s.map_field_weighting != weighting {
        physics.update(|state| state.map_field_weighting = weighting);
    }

    ui.add_space(8.0);
    ui.separator();
    let dose_scale = state.scale_for(MapBasis::DoseRate, s, 0.0);
    draw_dispersion_table(ui, s, dose_scale);
    action
}

#[cfg(test)]
mod tests {
    /// The NRC 2023 EPZ reference figure: `10e-3 Sv / 96 h` = 104.17 µSv/h,
    /// and it sits inside the dose-rate basis's default 0.8 µSv/h - 1 mSv/h
    /// scale (so the default ramp shows the tick). Floor and top unchanged.
    #[test]
    fn the_nrc_epz_reference_is_ten_millisievert_over_96_hours() {
        let r = super::nrc_2023_epz_reference_usv_per_h();
        assert!((r - 104.1666666667).abs() < 1e-6, "{r}");
        assert!((r - 104.17).abs() < 0.005);
        let scale = super::dose_rate_anchor_scale();
        assert!(r > scale.floor && r < scale.top());
        assert_eq!(super::FLOOR_USV_PER_H, 0.8);
        assert_eq!(super::TOP_USV_PER_H, 1000.0);
        assert!(super::dose_cell_text_with_reference(200.0, 0.8).ends_with('*'));
        assert!(!super::dose_cell_text_with_reference(50.0, 0.8).ends_with('*'));
    }
    use super::*;

    /// The rose's plot convention must match the physics module's site frame,
    /// and the plume arrow must point where the plume TRAVELS.
    ///
    /// Two separate sign traps in one picture. First, bearing-to-plot must put
    /// north at `+y` (sine on east, cosine on north) — swapping them mirrors
    /// the rose about the NE diagonal, which still looks like a plume. Second,
    /// the arrow is drawn at `wind_from + 180` because meteorological wind
    /// direction names where the wind comes *from*; drawing it at
    /// `wind_from` puts the plume arrow pointing back up the plume, which is
    /// the single most common error in a dispersion display.
    #[test]
    fn the_rose_convention_matches_the_physics_and_the_arrow_points_downwind() {
        use crate::physics::atmospheric_dispersion::bearing_to_site_frame;

        for bearing in [0.0, 45.0, 90.0, 180.0, 270.0, 315.0] {
            let (px, py) = bearing_to_plot(bearing, 1.0);
            let (sx, sy) = bearing_to_site_frame(bearing, 1.0);
            assert!(
                (px - sx).abs() < 1e-12 && (py - sy).abs() < 1e-12,
                "bearing {bearing}: rose ({px:.6}, {py:.6}) must match the site frame \
                 ({sx:.6}, {sy:.6})"
            );
        }

        // North at bearing 0 is +y.
        let (x, y) = bearing_to_plot(0.0, 1.0);
        assert!(x.abs() < 1e-12 && (y - 1.0).abs() < 1e-12);
        // East at bearing 90 is +x.
        let (x, y) = bearing_to_plot(90.0, 1.0);
        assert!((x - 1.0).abs() < 1e-12 && y.abs() < 1e-12);

        // A wind FROM the north (0 deg) must draw the plume arrow SOUTH.
        let travel = 0.0 + 180.0;
        let (ax, ay) = bearing_to_plot(travel, 1.0);
        assert!(
            ay < -0.9 && ax.abs() < 1e-9,
            "a north wind must point the plume arrow south; got ({ax:.3}, {ay:.3})"
        );
    }

    /// The colour scale must clamp above its top, grey out at and below its
    /// floor, never render a zero, a negative or a NaN as anything but that
    /// grey, and that grey must recede on the white ground.
    ///
    /// ~~`the_log_shading_spans_four_decades_and_clamps`~~ -- REPLACED
    /// 2026-09-28 with `log_shade` itself (see `ColourScale::shade`). Its
    /// contract carries over; the one change is that below-floor is now the
    /// grey rather than the bottom ramp colour, because the floor is a minimum
    /// reading. The four-decade convention is pinned by
    /// `the_chi_over_q_basis_keeps_the_four_decade_default`.
    #[test]
    fn the_colour_scale_clamps_and_greys_below_the_floor() {
        let scale = ColourScale::clamped(1.0e-9, 4.0);
        let top = scale.top();
        assert!((top - 1.0e-5).abs() < 1e-18);
        let at_top = scale.shade(top);
        assert_ne!(
            at_top,
            scale.shade(top / 10.0),
            "one decade must be visibly different"
        );
        assert_eq!(
            at_top,
            scale.shade(top * 1.0e20),
            "above the top the scale clamps"
        );
        for v in [0.0, -1.0, f64::NAN, 1.0e-9, 1.0e-12] {
            assert_eq!(
                scale.shade(v),
                NO_READING_GREY,
                "{v} is not above the floor"
            );
        }
        assert_ne!(NO_READING_GREY, at_top);
        let (r, g, b) = (
            NO_READING_GREY.r() as u16,
            NO_READING_GREY.g() as u16,
            NO_READING_GREY.b() as u16,
        );
        assert!(
            (r + g + b) / 3 >= 200,
            "the no-reading colour must recede on white"
        );
    }

    /// **The slider defaults are the cited anchors, and the arithmetic is the
    /// documented one.**
    ///
    /// # Methodology
    ///
    /// Coefficients: US EPA FGR-11 (EPA-520/1-88-020, 1988), Table 2.2,
    /// Effective column -- K-40 ingestion 5.02e-9 Sv/Bq (printed p. 156), Cs-137
    /// ingestion 1.35e-8 Sv/Bq (printed p. 166), read off the rendered pages
    /// 2026-09-28. The expected values are written out as literals here from
    /// the hand arithmetic in `reference/References.md`, so a changed constant
    /// fails the test rather than silently moving the anchor:
    ///
    /// ```text
    /// floor = 1.0e-7 Sv / 5.02e-9 Sv/Bq = 19.920 Bq      ("≈ banana")
    /// top   = 1.0 Sv    / 1.35e-8 Sv/Bq = 7.4074e7 Bq    ("≈ 1 Sv-equivalent")
    /// span  = log10(7.4074e7 / 19.920)  = 6.5704 decades
    /// ```
    ///
    /// # Results (2026-09-28)
    ///
    /// Pass. These are **indicative colour anchors**, not a dose calculation.
    #[test]
    fn the_slider_defaults_are_the_cited_anchors() {
        assert_eq!(FGR11_K40_INGESTION_SV_PER_BQ, 5.02e-9);
        assert_eq!(FGR11_CS137_INGESTION_SV_PER_BQ, 1.35e-8);
        assert!(
            (banana_anchor() - 19.920_318_7).abs() < 1e-6,
            "{}",
            banana_anchor()
        );
        assert!(
            (one_sievert_anchor() - 7.407_407_4e7).abs() < 1.0,
            "{}",
            one_sievert_anchor()
        );

        let a = anchor_scale();
        assert!((a.floor - banana_anchor()).abs() < 1e-12);
        assert!(
            (a.span_decades - 6.570_4).abs() < 1e-4,
            "{}",
            a.span_decades
        );
        assert!((a.top() / one_sievert_anchor() - 1.0).abs() < 1e-12);

        // The Absolute basis defaults to it, and a fresh tab state is on Absolute.
        let s = HtgrSnapshot::default();
        assert_eq!(default_scale(MapBasis::Absolute, &s, 1.0e-5), a);
        let state = MapTabState::default();
        assert_eq!(state.basis, MapBasis::Absolute);
        assert_eq!(state.scale_for(MapBasis::Absolute, &s, 1.0e-5), a);
        // Anchors sit inside the slider ranges, so the defaults are reachable.
        assert_eq!(ColourScale::clamped(a.floor, a.span_decades), a);
    }

    /// The two sliders clamp sensibly: out-of-range and non-finite inputs land
    /// on the range ends instead of producing a NaN or an inverted scale.
    #[test]
    fn the_scale_sliders_clamp_sensibly() {
        let lo = ColourScale::clamped(0.0, f64::NAN);
        assert_eq!(
            (lo.floor, lo.span_decades),
            (ColourScale::FLOOR_MIN, ColourScale::SPAN_MIN)
        );
        let neg = ColourScale::clamped(-5.0, -3.0);
        assert_eq!(
            (neg.floor, neg.span_decades),
            (ColourScale::FLOOR_MIN, ColourScale::SPAN_MIN)
        );
        let hi = ColourScale::clamped(1.0e40, 100.0);
        assert_eq!(
            (hi.floor, hi.span_decades),
            (ColourScale::FLOOR_MAX, ColourScale::SPAN_MAX)
        );
        let nan = ColourScale::clamped(f64::INFINITY, f64::INFINITY);
        assert!(nan.floor.is_finite() && nan.span_decades.is_finite());
        for sc in [lo, neg, hi, nan] {
            assert!(sc.top() > sc.floor, "top must lie above the floor: {sc:?}");
        }
    }

    /// Absolute falls back to Per Ci when the absolute rate is unavailable;
    /// Per Ci carries the anchors by the rate ratio when it can.
    #[test]
    fn absolute_falls_back_to_per_ci_and_per_ci_carries_the_anchors() {
        let mut s = HtgrSnapshot::default();
        // Before the first run both rates are NaN.
        assert_eq!(effective_basis(MapBasis::Absolute, &s), MapBasis::PerCi);
        assert_eq!(effective_basis(MapBasis::ChiOverQ, &s), MapBasis::ChiOverQ);
        s.dispersion_source_rate_absolute_bq_per_s = 2.0e6;
        s.dispersion_source_rate_per_ci_bq_per_s = 4.0e-4;
        assert_eq!(effective_basis(MapBasis::Absolute, &s), MapBasis::Absolute);
        let per_ci = default_scale(MapBasis::PerCi, &s, 1.0e-5);
        let a = anchor_scale();
        assert!((per_ci.floor - a.floor * 4.0e-4 / 2.0e6).abs() < 1e-18);
        assert_eq!(per_ci.span_decades, a.span_decades);
    }

    /// The chi/Q basis keeps the map's old default: four decades below the peak.
    #[test]
    fn the_chi_over_q_basis_keeps_the_four_decade_default() {
        let s = HtgrSnapshot::default();
        let sc = default_scale(MapBasis::ChiOverQ, &s, 1.0e-5);
        assert!((sc.floor - 1.0e-9).abs() < 1e-21 && sc.span_decades == 4.0);
    }

    /// A field wholly under the floor must say so on the map instead of
    /// painting a blank square that reads as "no plume".
    #[test]
    fn a_field_under_the_floor_is_announced() {
        let a = anchor_scale();
        assert!(below_floor_note(a.floor * 10.0, a, MapBasis::Absolute).is_none());
        let note = below_floor_note(1.7e-4, a, MapBasis::Absolute).expect("under the floor");
        assert!(
            note.contains("5.1 decades") && note.contains("Bq/m^3"),
            "{note}"
        );
        assert!(below_floor_note(0.0, a, MapBasis::PerCi).is_some());
        assert!(below_floor_note(f64::NAN, a, MapBasis::Absolute).is_none());
    }

    /// The per-Ci table's columns are unchanged (maintainer direction
    /// 2026-09-28), and every quantity in the absolute table carries its
    /// true unit.
    #[test]
    fn the_per_ci_table_is_unchanged() {
        assert_eq!(
            PER_CI_TABLE_HEADINGS,
            [
                "Bearing",
                "Distance",
                "chi/Q LIVE [s/m^3]",
                "chi/Q integrated [s/m^3]",
                "Air [Bq.s/m^3 per Ci]",
                "Ground [Bq/m^2 per Ci]",
            ]
        );
        assert_eq!(ABSOLUTE_TABLE_HEADINGS[2], "Air LIVE [Bq/m^3]");
        assert_eq!(ABSOLUTE_TABLE_HEADINGS[3], "Air integrated [Bq.s/m^3]");
        assert_eq!(ABSOLUTE_TABLE_HEADINGS[4], "Ground, dry [Bq/m^2]");
    }

    /// **The chi/Q table's values are the dilution factors the map uses, and
    /// the absolute LIVE column is the Absolute-basis pixel value.**
    ///
    /// # Methodology
    ///
    /// Run the real dispersion channel (1200 K kernel release, default
    /// meteorology), march the plume 600 s with `refresh_field`, evaluate,
    /// and project the result onto a snapshot exactly as
    /// `physics::write_snapshot` does. Then:
    ///
    /// 1. each LIVE cell of [`chi_over_q_rows`] equals the receptor's
    ///    instantaneous `chi/Q` (bit-for-bit), which `evaluate` fills from
    ///    `instantaneous_chi_over_q_at_ring` --
    ///    the same kernel and population the map field is painted from, which
    ///    `atmospheric_dispersion::tests::the_live_ring_sample_agrees_with_the_field_cell_under_it`
    ///    ties to the grid cell under the receptor;
    /// 2. each integrated cell equals the receptor's time-integrated `chi/Q`;
    /// 3. the absolute LIVE value ([`receptor_air_absolute`], the
    ///    emission-weighted sum the Absolute texture paints) equals
    ///    `chi/Q x source_rate_absolute` within one f32 epsilon (1.19e-7)
    ///    relative, the f32 storage of the weighted puff weight (measured
    ///    2026-09-29: 1.1e-8), plus `F32_SUBNORMAL_FLOOR` absolute for
    ///    subnormal tail receptors -- equal only because this run holds the release
    ///    at one rate (gh:#400: with a moving rate the two differ, and the
    ///    weighted sum is the right one).
    ///
    /// # Results (2026-09-29, gh:#400)
    ///
    /// Pass: 3 distances x 8 bearings. Printed: peak LIVE `chi/Q` on the ring
    /// 1.805e-5 s/m^3 (unchanged), absolute **stack** release rate 58.69 Bq/s
    /// (with the gh:#400 building credit; not re-measured since the building
    /// was taken off the default path, gh:#409; pending validation work),
    /// peak absolute LIVE air concentration **1.059e-3 Bq/m^3** -- about
    /// **4.3 decades below** the default "≈ banana" floor (19.92).
    /// ~~9.515 Bq/s and 1.717e-4 Bq/m^3, 5.1 decades below~~ (2026-09-28):
    /// that rate was circulating activity x a flat 1e-6/s leak fraction; the
    /// rate is now the live primary pools of gh:#399 leaking into the bishan
    /// building and out of its stack (gh:#400). Interpretation unchanged: on
    /// the default scale a normal-operation plume at this kernel temperature
    /// paints nothing; `below_floor_note` says so on the map.
    #[test]
    fn the_chi_over_q_table_is_the_maps_dilution_factor() {
        use crate::physics::atmospheric_dispersion::AtmosphericDispersionChannel;
        use crate::physics::fission_product_release::TrisoAtopsReleaseChannel;
        use uom::si::f64::ThermodynamicTemperature;
        use uom::si::thermodynamic_temperature::kelvin;

        let mut release = TrisoAtopsReleaseChannel::new_htr10();
        release.update(
            0.0,
            Some(TrisoAtopsReleaseChannel::kernel_and_graphite(
                ThermodynamicTemperature::new::<kelvin>(1200.0),
                ThermodynamicTemperature::new::<kelvin>(950.0),
            )),
        );
        let mut channel = AtmosphericDispersionChannel::new();
        // `update` stamps the emission rates the puffs then carry (gh:#400).
        assert!(channel.update(0.0, &release));
        channel.refresh_field(600.0);
        let result = channel.evaluate(600.0, &release);

        let mut s = HtgrSnapshot::default();
        for (slot, r) in s.receptors.iter_mut().zip(result.receptors.iter()) {
            slot.bearing_deg = r.bearing_deg;
            slot.distance_m = r.distance_m;
            slot.chi_over_q = r.chi_over_q;
            slot.instantaneous_chi_over_q = r.instantaneous_chi_over_q;
            slot.air_bq_s_per_m3 = r.air_bq_s_per_m3;
            slot.ground_bq_per_m2 = r.ground_bq_per_m2;
            slot.air_bq_s_per_m3_absolute = r.air_bq_s_per_m3_absolute.unwrap_or(f64::NAN);
            slot.ground_bq_per_m2_absolute = r.ground_bq_per_m2_absolute.unwrap_or(f64::NAN);
            slot.instantaneous_air_bq_per_m3_by_nuclide = r.instantaneous_air_bq_per_m3_by_nuclide;
        }
        s.dispersion_source_rate_per_ci_bq_per_s = result.source_rate_per_ci_bq_per_s;
        s.dispersion_source_rate_absolute_bq_per_s =
            result.source_rate_absolute_bq_per_s.unwrap_or(f64::NAN);

        let rows = chi_over_q_rows(&s);
        assert_eq!(rows.len(), 3, "one row per receptor distance");
        let mut checked = 0;
        for row in &rows {
            assert_eq!(row.bearings_deg.len(), 8, "all eight bearings");
            for (k, bearing) in row.bearings_deg.iter().enumerate() {
                let r = result
                    .receptors
                    .iter()
                    .find(|r| {
                        (r.distance_m - row.distance_m).abs() < 1e-9
                            && (r.bearing_deg - bearing).abs() < 1e-9
                    })
                    .expect("receptor in the ring");
                assert_eq!(row.live[k], r.instantaneous_chi_over_q);
                assert_eq!(row.integrated[k], r.chi_over_q);
                let slot = s
                    .receptors
                    .iter()
                    .find(|x| {
                        (x.distance_m - r.distance_m).abs() < 1e-9
                            && (x.bearing_deg - r.bearing_deg).abs() < 1e-9
                    })
                    .expect("projected receptor");
                let abs_live = receptor_air_absolute(slot);
                let constant_rate =
                    r.instantaneous_chi_over_q * result.source_rate_absolute_bq_per_s.unwrap();
                assert!(
                    (abs_live - constant_rate).abs()
                        <= f32::EPSILON as f64 * constant_rate.abs() + F32_SUBNORMAL_FLOOR,
                    "{abs_live} vs {constant_rate}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 24);
        let peak_live = rows
            .iter()
            .flat_map(|r| r.live.iter())
            .copied()
            .fold(0.0, f64::max);
        println!(
            "peak LIVE chi/Q on the ring {peak_live:.3e} s/m^3; absolute rate {:.3e} Bq/s; \
             peak absolute LIVE air {:.3e} Bq/m^3 vs default floor {:.3e}",
            s.dispersion_source_rate_absolute_bq_per_s,
            peak_live * s.dispersion_source_rate_absolute_bq_per_s,
            anchor_scale().floor
        );
        assert!(
            rows.iter().flat_map(|r| r.live.iter()).any(|v| *v > 0.0),
            "the plume must have reached at least one receptor after 600 s"
        );
    }

    /// The downwind filter must select the half-plane the plume is actually
    /// in, and must wrap correctly around north.
    ///
    /// The wrap is the interesting case: with a wind from 90 deg the plume
    /// travels to 270 deg, and the downwind sectors are 180-360 — but with a
    /// wind from 270 the plume travels to 90 and the sectors straddle 0, where
    /// a naive difference would exclude exactly the sectors it should keep.
    #[test]
    fn the_downwind_filter_wraps_around_north() {
        // Plume travelling due north (0 deg): 315 and 45 are both within 90,
        // and they straddle the wrap.
        assert!(angular_distance(315.0, 0.0) <= 90.0);
        assert!(angular_distance(45.0, 0.0) <= 90.0);
        assert!(angular_distance(180.0, 0.0) > 90.0);

        // Symmetric, and never greater than 180.
        for (a, b) in [(10.0, 350.0), (0.0, 180.0), (90.0, 270.0), (359.0, 1.0)] {
            let ab = angular_distance(a, b);
            assert!(
                (ab - angular_distance(b, a)).abs() < 1e-12,
                "must be symmetric"
            );
            assert!((0.0..=180.0).contains(&ab), "got {ab} for ({a}, {b})");
        }
        assert!((angular_distance(359.0, 1.0) - 2.0).abs() < 1e-12);
    }

    /// The resolution request must be in PHYSICAL PIXELS, and must not
    /// re-fire on every pixel of a window drag.
    ///
    /// Both halves have teeth. Asking in points on a 2x display would paint
    /// every evaluated cell across four pixels -- the "big box" the
    /// single-pixel direction replaces, only smaller and harder to notice.
    /// And a request that changed on every intermediate width would
    /// invalidate the field cache on every frame of a resize, turning a drag
    /// into a sustained recomputation.
    #[test]
    fn the_resolution_request_is_in_pixels_and_has_a_dead_band() {
        assert_eq!(requested_cells(320.0, 1.0), 320);
        assert_eq!(
            requested_cells(320.0, 2.0),
            640,
            "HiDPI must ask for real pixels"
        );
        assert_eq!(
            requested_cells(0.0, 1.0),
            1,
            "a degenerate size must not be zero"
        );

        // A one-pixel wobble on a 500-cell map is inside the dead band; a
        // real resize is not.
        assert!(!resolution_request_changed(500, 501));
        assert!(!resolution_request_changed(500, 495));
        assert!(resolution_request_changed(500, 560));
        // The dead band has a floor, so a small map still responds.
        assert!(resolution_request_changed(64, 80));
    }

    /// The displaced-plume-clock banner must say EXTRAPOLATION forwards and
    /// RESTART backwards, and must stay silent when the clocks agree.
    ///
    /// # Methodology
    ///
    /// [`plume_offset_banner`] is called with the three cases the control can
    /// produce -- `0.0` (the "Now" state), `+3600` (one `+1 h` press) and
    /// `-3600` (one `-1 h` press) -- and the returned string is checked for the
    /// words that carry the two limitations gh:#344 introduced:
    ///
    /// 1. forwards, the field was **marched under the current wind**, so the
    ///    banner must say `EXTRAPOLATION` and `CURRENT wind` and must not
    ///    promise a forecast;
    /// 2. backwards, the population was **cleared**, so the banner must say
    ///    `CLEARED` and `RESTARTED` rather than repeating the forward wording.
    ///
    /// The pass criterion is a substring match, not an exact string, so
    /// rewording the banner does not fail the test while deleting either
    /// limitation does.
    ///
    /// # Results (measured 2026-09-27, this run)
    ///
    /// | Offset | Banner |
    /// |---|---|
    /// | `0.0` | `None` |
    /// | `+3600` | `"Plume clock is running 1:00:00 AHEAD of the plant. The map is an EXTRAPOLATION: … marched under the CURRENT wind …"` |
    /// | `-3600` | `"Plume clock is 1:00:00 BEHIND the plant. Rewinding CLEARED the puff population … the plume has RESTARTED from the stack …"` |
    ///
    /// All assertions pass. **Interpretation:** this is a *GUI-text* guard, not
    /// physics V&V -- it checks that the two honest caveats reach the screen,
    /// and it would have failed against the banner as it stood before this
    /// change, which said `AHEAD` for both signs and claimed exactness.
    #[test]
    fn the_offset_banner_says_extrapolation_forwards_and_restart_backwards() {
        assert!(
            plume_offset_banner(0.0).is_none(),
            "no banner when the plume clock is on the plant clock"
        );

        let ahead = plume_offset_banner(3600.0).expect("a displaced clock must warn");
        assert!(ahead.contains("AHEAD"), "got {ahead}");
        assert!(
            ahead.contains("1:00:00"),
            "the magnitude must be readable; got {ahead}"
        );
        assert!(
            ahead.contains("EXTRAPOLATION") && ahead.contains("CURRENT wind"),
            "a forward jump is marched under the current wind and must say so; got {ahead}"
        );

        let behind = plume_offset_banner(-3600.0).expect("a rewound clock must warn");
        assert!(
            behind.contains("BEHIND"),
            "a rewind is not a fast-forward; got {behind}"
        );
        assert!(
            behind.contains("CLEARED") && behind.contains("RESTARTED"),
            "a rewind clears the population and must say so; got {behind}"
        );
        assert!(
            !behind.contains("AHEAD"),
            "the old banner said AHEAD for both signs; got {behind}"
        );
    }

    /// The clock readout must be `h:mm:ss` and must not panic on the `NAN`
    /// the snapshot carries before the first field.
    /// An accident button is enabled only once its stage lands: water
    /// ingress (gh:#401) is live and starts its scenario; DLOFC (gh:#402)
    /// still waits. Flip `enabled` in the change that lands the physics, and
    /// this test with it.
    #[test]
    fn the_accident_buttons_wait_for_their_stages() {
        let labels: Vec<&str> = ACCIDENT_BUTTONS.iter().map(|b| b.label).collect();
        assert_eq!(labels, ["Water ingress", "DLOFC + ATWS + air ingress"]);
        assert!(ACCIDENT_BUTTONS[0].enabled && !ACCIDENT_BUTTONS[1].enabled);
        assert_eq!(ACCIDENT_BUTTONS[0].action, MapAction::StartWaterIngress);
        assert!(ACCIDENT_BUTTONS[0].hover.contains("gh:#401"));
        assert!(ACCIDENT_BUTTONS[1].hover.contains("gh:#402"));
    }

    #[test]
    fn the_clock_reads_out_in_hours_minutes_seconds() {
        assert_eq!(clock_text(0.0), "0:00:00");
        assert_eq!(clock_text(59.4), "0:00:59");
        assert_eq!(clock_text(3600.0), "1:00:00");
        assert_eq!(clock_text(7265.0), "2:01:05");
        assert_eq!(clock_text(f64::NAN), "--");
        assert_eq!(clock_text(-5.0), "0:00:00");
    }

    // ----- Dose-rate basis (2026-09-29) ---------------------------------

    /// Absolute allowance for a receptor deep in a plume's tail, where the
    /// per-puff f32 contribution falls below `f32::MIN_POSITIVE` (1.18e-38)
    /// and keeps no relative precision (subnormals): one normal-floor unit per
    /// puff for up to 1000 puffs (the map population is ~120). Fixed by the
    /// f32 representation, not by any comparison.
    const F32_SUBNORMAL_FLOOR: f64 = 1.0e3 * f32::MIN_POSITIVE as f64;

    /// A snapshot projected from a real dispersion run on the MAP's puff
    /// model, exactly as `physics::write_snapshot` projects it, with the
    /// field summed on `basis` (gh:#400). The release is held at one rate
    /// from t = 0, so every puff carries the same emission rates.
    fn map_snapshot(kernel_k: f64, basis: MapBasis) -> HtgrSnapshot {
        use crate::physics::atmospheric_dispersion::AtmosphericDispersionChannel;
        use crate::physics::fission_product_release::TrisoAtopsReleaseChannel;
        use uom::si::f64::ThermodynamicTemperature;
        use uom::si::thermodynamic_temperature::kelvin;

        let mut release = TrisoAtopsReleaseChannel::new_htr10();
        release.update(
            0.0,
            Some(TrisoAtopsReleaseChannel::kernel_and_graphite(
                ThermodynamicTemperature::new::<kelvin>(kernel_k),
                ThermodynamicTemperature::new::<kelvin>(950.0),
            )),
        );
        let mut channel = AtmosphericDispersionChannel::new();
        channel.set_meteorology(crate::physics::map_puff_model::map_meteorology(1.0, 0.0));
        let mut request = channel.field_request();
        request.weighting = basis.weighting();
        channel.set_field_request(request);
        // `update` stamps the emission rates the puffs then carry.
        assert!(channel.update(0.0, &release));
        channel.refresh_field(1200.0);
        let result = channel.evaluate(1200.0, &release);
        let mut s = HtgrSnapshot::default();
        for (slot, r) in s.receptors.iter_mut().zip(result.receptors.iter()) {
            slot.bearing_deg = r.bearing_deg;
            slot.distance_m = r.distance_m;
            slot.chi_over_q = r.chi_over_q;
            slot.instantaneous_chi_over_q = r.instantaneous_chi_over_q;
            slot.air_bq_s_per_m3_absolute = r.air_bq_s_per_m3_absolute.unwrap_or(f64::NAN);
            slot.ground_bq_per_m2_absolute = r.ground_bq_per_m2_absolute.unwrap_or(f64::NAN);
            slot.ground_bq_per_m2_absolute_by_nuclide = r.ground_bq_per_m2_absolute_by_nuclide;
            slot.instantaneous_air_bq_per_m3_by_nuclide = r.instantaneous_air_bq_per_m3_by_nuclide;
        }
        s.dispersion_source_rate_absolute_bq_per_s =
            result.source_rate_absolute_bq_per_s.unwrap_or(f64::NAN);
        s.dispersion_source_rate_absolute_by_nuclide_bq_per_s = result
            .source_rate_absolute_by_nuclide_bq_per_s
            .map(|r| r.unwrap_or(f64::NAN));
        s.dispersion_grid = result.grid.values.iter().map(|v| *v as f32).collect();
        s.dispersion_grid_weighting = result.grid.weighting;
        s.dispersion_grid_cells = result.grid.cells;
        s
    }

    /// **The dose-rate table's air row is the map pixel, and the pixel is the
    /// sum of the buangkok pathway functions.**
    ///
    /// # Methodology
    ///
    /// Real dispersion run on the map's puff model (inter-monsoon, class B,
    /// 1 m/s, from 0 deg), 1200 K kernel, settled 1200 s. For every receptor:
    ///
    /// 1. the table's air cell equals the receptor's emission-weighted
    ///    air dose rate ([`receptor_air_dose_rate`]) bit for bit and, the
    ///    release being held at one rate, `live chi/Q x today's dose factor`
    ///    (the pre-gh:#400 formula, which is exact only in this constant-rate
    ///    case) within one f32 epsilon (1.19e-7) relative -- the weighted
    ///    puff weight is stored as f32 (`changi::puff::wgsl::PuffState`), one
    ///    rounding of at most half an epsilon per puff, fixed by the storage
    ///    and not by this comparison; measured 2026-09-29: 1.4e-8 -- plus
    ///    `F32_SUBNORMAL_FLOOR` absolute for tail receptors whose per-puff f32
    ///    contribution is subnormal (one at ~8e-42 µSv/h differs at 6e-6
    ///    relative);
    /// 2. that equals the split table's submersion + inhalation totals
    ///    (buangkok functions called per nuclide) within 1e-12 relative
    ///    (f64 reassociation only);
    /// 3. the ground row equals the split table's ground-shine total.
    ///
    /// # Results (2026-09-29, re-measured after gh:#400)
    ///
    /// Not re-measured since the building credit was removed from the default
    /// path (gh:#409); pending validation work. Pass. At the 1200 K default:
    /// absolute stack release 58.69 Bq/s, peak
    /// ring air dose rate 4.769e-8 µSv/h, peak field air 6.659e-8 µSv/h (7.1
    /// decades under the 0.8 floor), peak ring ground shine 2.399e-11 µSv/h;
    /// absolute peak field concentration 4.437e-3 Bq/m^3. Kernel sweep, peak
    /// field air: 1400 K 2.542e-7, 1600 K 2.348e-5, 1800 K 1.195e-4, 2000 K
    /// 4.463e-4 µSv/h. ~~1.625e-7 / 2.269e-7 / 1.140e-10 µSv/h, 7.194e-4
    /// Bq/m^3, 2000 K 3.048e-6 µSv/h~~ (earlier 2026-09-29, before the
    /// gh:#399 live pools and the gh:#400 building): more activity leaves the
    /// stack now (58.7 vs 9.5 Bq/s) yet the air dose rate at 1200 K fell by
    /// 3.4x, so the per-nuclide mix moved towards the low-coefficient noble
    /// gases; the shift is not decomposed nuclide by nuclide here. The table
    /// in `reference/References.md` ("Dose-rate basis") carries both.
    #[test]
    fn the_dose_rate_table_is_the_pixel_and_the_buangkok_sum() {
        let s = map_snapshot(1200.0, MapBasis::DoseRate);
        let rows = dose_rate_rows(&s);
        assert_eq!(rows.len(), 3);
        let c = dose_rate::coefficients();
        let mut peak_air: f64 = 0.0;
        let mut peak_ground: f64 = 0.0;
        for row in &rows {
            for (k, b) in row.bearings_deg.iter().enumerate() {
                let r = s
                    .receptors
                    .iter()
                    .find(|r| {
                        (r.distance_m - row.distance_m).abs() < 1e-9
                            && (r.bearing_deg - b).abs() < 1e-9
                    })
                    .unwrap();
                assert_eq!(row.air[k], receptor_air_dose_rate(r));
                let constant_rate = r.instantaneous_chi_over_q
                    * dose_rate::air_dose_rate_per_unit_chi_over_q(
                        &s.dispersion_source_rate_absolute_by_nuclide_bq_per_s,
                        c,
                    );
                assert!(
                    (row.air[k] - constant_rate).abs()
                        <= f32::EPSILON as f64 * constant_rate.abs() + F32_SUBNORMAL_FLOOR,
                    "{} vs {constant_rate}",
                    row.air[k]
                );
                let split = dose_rate::receptor_split(
                    1.0,
                    &r.instantaneous_air_bq_per_m3_by_nuclide,
                    &r.ground_bq_per_m2_absolute_by_nuclide,
                    c,
                );
                let sum = dose_rate::pathway_total(&split, Pathway::Submersion).0
                    + dose_rate::pathway_total(&split, Pathway::Inhalation).0;
                if sum > 0.0 {
                    assert!(
                        ((row.air[k] - sum) / sum).abs() <= 1e-12,
                        "{} vs {sum}",
                        row.air[k]
                    );
                } else {
                    assert_eq!(row.air[k], 0.0);
                }
                assert_eq!(
                    row.ground[k],
                    dose_rate::pathway_total(&split, Pathway::GroundShine).0
                );
                peak_air = peak_air.max(row.air[k]);
                peak_ground = peak_ground.max(row.ground[k]);
            }
        }
        for kernel in [1400.0, 1600.0, 1800.0, 2000.0] {
            let h = map_snapshot(kernel, MapBasis::DoseRate);
            let p = field_value(field_peak_sample(&h), MapBasis::DoseRate, &h);
            let g = dose_rate_rows(&h)
                .iter()
                .flat_map(|r| r.ground.iter())
                .copied()
                .fold(0.0, f64::max);
            println!(
                "kernel {kernel} K: absolute release {:.3e} Bq/s, peak field air dose rate \
                 {p:.3e} uSv/h, peak ring ground shine {g:.3e} uSv/h",
                h.dispersion_source_rate_absolute_bq_per_s
            );
        }
        let field_peak = field_value(field_peak_sample(&s), MapBasis::DoseRate, &s);
        let a = map_snapshot(1200.0, MapBasis::Absolute);
        println!(
            "default (1200 K kernel) map puff model: absolute release {:.3e} Bq/s, peak field \
             air concentration {:.3e} Bq/m^3",
            s.dispersion_source_rate_absolute_bq_per_s,
            field_value(field_peak_sample(&a), MapBasis::Absolute, &a)
        );
        println!(
            "default (1200 K kernel) map puff model: peak ring air dose rate {peak_air:.3e} uSv/h, \
             peak field air {field_peak:.3e} uSv/h, peak ring ground shine {peak_ground:.3e} uSv/h, \
             floor {FLOOR_USV_PER_H} uSv/h ({:.1} decades below)",
            (FLOOR_USV_PER_H / field_peak).log10()
        );
        assert!(peak_air > 0.0);
    }

    /// The dose-rate basis's defaults are the maintainer's anchors, and a
    /// fresh tab's untouched dose-rate scale is that.
    #[test]
    fn the_dose_rate_defaults_are_the_maintainers_anchors() {
        let a = dose_rate_anchor_scale();
        assert_eq!(a.floor, 0.8);
        assert!((a.top() - 1000.0).abs() < 1e-9, "{}", a.top());
        assert!((a.span_decades - (1000.0_f64 / 0.8).log10()).abs() < 1e-15);
        let s = HtgrSnapshot::default();
        assert_eq!(
            MapTabState::default().scale_for(MapBasis::DoseRate, &s, 1.0),
            a
        );
        assert_eq!(MapBasis::DoseRate.unit(), "µSv/h");
        assert!(DOSE_RATE_FRAMING.contains("not a dose to any real person"));
        assert!(DOSE_RATE_FRAMING.contains("not for emergency or regulatory use"));
    }

    /// Missing coefficients show as missing, below-floor cells keep their
    /// number and say so, and nothing unavailable prints as 0.
    #[test]
    fn missing_and_below_floor_cells_are_never_zero() {
        assert_eq!(dose_cell_text(f64::NAN, 0.8), "unavailable");
        assert_eq!(dose_cell_text(3.1e-9, 0.8), "3.10e-9 (below floor)");
        assert_eq!(dose_cell_text(2.0, 0.8), "2.000e0");
        assert!(dose_rate_missing_note().contains("Kr-85 Inhalation (committed)"));
        assert!(dose_rate_missing_note().contains("not counted as zero"));
        // No absolute rate yet: the dose factor is unavailable, not zero.
        let s = HtgrSnapshot::default();
        assert!(MapBasis::DoseRate.factor(&s).is_nan());
        assert_eq!(effective_basis(MapBasis::DoseRate, &s), MapBasis::DoseRate);
    }

    /// A dose-rate field wholly below the 0.8 µSv/h floor is announced, with
    /// the reason, on the map and under the table.
    #[test]
    fn a_below_floor_dose_rate_field_is_announced() {
        let a = dose_rate_anchor_scale();
        let note = below_floor_note(1e-9, a, MapBasis::DoseRate).expect("below floor");
        assert!(
            note.contains("normal-operation release is tiny") && note.contains("µSv/h"),
            "{note}"
        );
        assert!(below_floor_note(5.0, a, MapBasis::DoseRate).is_none());
        let s = map_snapshot(1200.0, MapBasis::DoseRate);
        let rows = dose_rate_rows(&s);
        let table_note = dose_table_below_floor_note(&rows, a.floor)
            .expect("default release is below the floor");
        assert!(table_note.contains("normal-operation release is tiny"));
    }

    /// Same inputs, byte-identical dose-rate table: the dose-rate path adds no
    /// wall clock, RNG or I/O dependence (the headless-determinism property).
    #[test]
    fn the_dose_rate_table_is_deterministic() {
        let a = dose_rate_rows(&map_snapshot(1200.0, MapBasis::DoseRate));
        let b = dose_rate_rows(&map_snapshot(1200.0, MapBasis::DoseRate));
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
