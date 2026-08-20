//! The plot-independent data model: a thermodynamic point, and a named layer of
//! them.
//!
//! # Why one model feeds all four diagrams
//!
//! A T-p, p-h, T-s and h-s diagram are four projections of the *same*
//! thermodynamic surface. Computing a saturation dome four times, once per tab,
//! would be four chances to compute it four slightly different ways. Instead a
//! layer is a set of [`ThermoPoint`]s — each carrying `p`, `T`, `h`, `s` and,
//! where it is meaningful, the vapour quality — and each diagram simply picks
//! the two coordinates it wants. That also means the CSV export can write the
//! **full** state of every plotted point rather than the two numbers that
//! happened to be on screen.
//!
//! # Units
//!
//! Every field is a `uom` dimensioned quantity, per the crate convention: no
//! bare `f64` SI values cross a physics boundary. Conversion to plot units
//! (bar, °C, kJ/kg, kJ/(kg K)) happens once, in [`crate::diagram`], at the
//! moment a point is projected onto axes.

use uom::si::f64::*;

use crate::figure::{Rgb, SeriesStyle};

/// One fully-specified point on the water/steam thermodynamic surface.
///
/// # Fields and their valid ranges
///
/// * `pressure` — absolute pressure. IAPWS-IF97 is defined from the triple-point
///   pressure 611.657 Pa to 100 MPa.
/// * `temperature` — thermodynamic temperature, 273.15 K to 1073.15 K for
///   Regions 1–3 and up to 2273.15 K in Region 5.
/// * `specific_enthalpy` — J/kg. (`uom` spells this `AvailableEnergy`; this
///   crate uses that type for specific enthalpy throughout.)
/// * `specific_entropy` — J/(kg K). (`uom` spells this `SpecificHeatCapacity`.)
/// * `quality` — vapour mass fraction, `Some(x)` with `0 <= x <= 1` only for a
///   two-phase (Region 4) state, `None` otherwise. See the caveat below.
/// * `reference_mass_flux` — carried, when a reference dataset reports one, so
///   the CSV export can pair the state with the measured critical mass flux it
///   came from. It is never plotted as a coordinate.
///
/// # Quality is derived, not validated
///
/// Where `quality` is `Some(x)` it was obtained from the Region-4 lever rule
///
/// ```text
/// x = (h - h_f(p)) / (h_g(p) - h_f(p))
/// ```
///
/// exactly as GitHub issue #26 specifies. Wagner and Kretzschmar do not
/// independently tabulate quality for every `(p, h)` state, so **quality here is
/// a derived quantity and is not an independently validated property** of this
/// implementation. Every diagram that draws quality lines repeats that
/// statement in a footnote on the figure itself.
#[derive(Clone, Copy, Debug)]
pub struct ThermoPoint {
    /// Absolute pressure.
    pub pressure: Pressure,
    /// Thermodynamic temperature.
    pub temperature: ThermodynamicTemperature,
    /// Specific enthalpy (J/kg).
    pub specific_enthalpy: AvailableEnergy,
    /// Specific entropy (J/(kg K)).
    pub specific_entropy: SpecificHeatCapacity,
    /// Vapour quality by the Region-4 lever rule, for two-phase states only.
    pub quality: Option<f64>,
    /// Reference critical mass flux reported alongside this state by its
    /// source dataset, if any. Never plotted; exported to CSV.
    pub reference_mass_flux: Option<MassFlux>,
}

impl ThermoPoint {
    /// Builds a point from its four state variables plus an optional quality.
    pub fn new(
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
        specific_enthalpy: AvailableEnergy,
        specific_entropy: SpecificHeatCapacity,
        quality: Option<f64>,
    ) -> Self {
        Self {
            pressure,
            temperature,
            specific_enthalpy,
            specific_entropy,
            quality,
            reference_mass_flux: None,
        }
    }

    /// Attaches a reference mass flux for the CSV export.
    pub fn with_reference_mass_flux(mut self, g: MassFlux) -> Self {
        self.reference_mass_flux = Some(g);
        self
    }

    /// Whether every state variable is finite. Guards against a flash that
    /// converged to nonsense leaking into a figure.
    pub fn is_finite(&self) -> bool {
        use uom::si::available_energy::joule_per_kilogram;
        use uom::si::pressure::pascal;
        use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;
        self.pressure.get::<pascal>().is_finite()
            && self.temperature.get::<kelvin>().is_finite()
            && self
                .specific_enthalpy
                .get::<joule_per_kilogram>()
                .is_finite()
            && self
                .specific_entropy
                .get::<joule_per_kilogram_kelvin>()
                .is_finite()
    }
}

/// Whether a layer is a computed curve or a set of reference data points.
///
/// This distinction is load-bearing, not cosmetic: **curves are computed live
/// from this crate's IAPWS-IF97 routines and points are cited measurements**.
/// The CSV export writes the two kinds with different provenance columns, and
/// the on-screen legend styles them differently, so a reader can never mistake
/// one for the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    /// Computed live from `tampines-steam-tables`.
    ComputedCurve,
    /// Measured, digitised or published reference data.
    ReferencePoints,
}

impl LayerKind {
    /// The word used in the CSV `kind` column.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ComputedCurve => "computed_curve",
            Self::ReferencePoints => "reference_points",
        }
    }
}

/// A named, styled set of points, split into pen-down runs.
///
/// `segments` is a list of *contiguous* runs. A curve that is genuinely
/// discontinuous — an isobar crossing the saturation dome, an isotherm jumping
/// across the vapour-pressure line — is several segments, never one segment with
/// an invented joining leg.
#[derive(Clone, Debug)]
pub struct PlotLayer {
    /// Legend and CSV name.
    pub label: String,
    /// Computed curve or reference data.
    pub kind: LayerKind,
    /// Citation, carried into the CSV so exported points keep their source.
    pub provenance: String,
    /// Contiguous runs of points.
    pub segments: Vec<Vec<ThermoPoint>>,
    /// How it is drawn.
    pub style: SeriesStyle,
    /// Colour.
    pub colour: Rgb,
    /// Whether it gets its own legend entry (a fan of sibling curves shares
    /// one).
    pub show_in_legend: bool,
}

impl PlotLayer {
    /// Total number of points across all segments.
    pub fn point_count(&self) -> usize {
        self.segments.iter().map(Vec::len).sum()
    }
}
