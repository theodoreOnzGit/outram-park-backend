//! The layer registry: what can be drawn, on which diagram, and why not.
//!
//! # The availability rule
//!
//! Issue #26: *"If some datasets are unavailable, the corresponding layer
//! should be disabled rather than filled with invented data."* This module is
//! where that rule is mechanised. [`LayerId::availability_on`] answers, for
//! every layer and every diagram, either [`Availability::Available`] or
//! [`Availability::Unavailable`] **with a reason string**, and the reason is
//! shown next to the greyed-out checkbox in the GUI. A layer is never quietly
//! absent and never quietly filled in.
//!
//! Two distinct things make a layer unavailable, and the reasons say which:
//!
//! * **The data does not exist.** The Edwards–O'Brien GS-1 trace is a measured
//!   *pressure* history; no enthalpy, entropy or quality was measured, and this
//!   crate's own blowdown trajectory is a simulation output rather than data.
//!   So it appears on the T-p diagram and nowhere else.
//! * **The curve would be degenerate.** An isobar on a p-h diagram is a
//!   horizontal line; an isotherm on a T-s diagram is a horizontal line; in T-p
//!   coordinates the whole two-phase region collapses onto the vapour-pressure
//!   curve, so the saturated-liquid line, the saturated-vapour line and all five
//!   quality lines are the same curve. Drawing those would imply structure that
//!   is not there.

use tampines_steam_tables::interfaces::checked::{try_s_ph_eqm, try_t_ph_eqm, try_x_ph_flash};
use tampines_steam_tables::region_1_subcooled_liquid::{h_tp_1, s_tp_1};
use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::p_boundary_2_3;
use tampines_steam_tables::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};
use uom::si::area::square_foot;
use uom::si::available_energy::{btu_it_per_pound, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::mass_rate::pound_per_second;
use uom::si::pressure::{bar, kilopascal, pascal, pound_force_per_square_inch};
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, degree_fahrenheit, kelvin};

use tampines_steam_tables::tabulated_data::{TabulatedData, TabulatedState};

use crate::curves;
use crate::data::{LayerKind, PlotLayer, ThermoPoint};
use crate::diagram::DiagramKind;
use crate::figure::{MarkerShape, Rgb, SeriesStyle, INK, PALETTE};
use crate::reference_data::{edwards, marviken, moody, wagner, zaloudek};

/// Whether a layer can be drawn on a given diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The layer applies and has real data behind it.
    Available,
    /// The layer does not apply here. The string says why, and is shown to the
    /// user rather than being swallowed.
    Unavailable(&'static str),
}

impl Availability {
    /// Convenience predicate.
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    /// The reason, if unavailable.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::Available => None,
            Self::Unavailable(why) => Some(why),
        }
    }
}

/// Every toggleable layer.
///
/// The first block is computed live from IAPWS-IF97; the second is cited
/// reference data. An enum, so adding a layer forces every `match` — the
/// availability table, the builder and the default-on set — to be updated
/// together, per the workspace Rust design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerId {
    /// The closed saturation envelope (liquid line up, vapour line back down).
    SaturationDome,
    /// The saturated-liquid line, `x = 0`.
    SaturatedLiquidLine,
    /// The saturated-vapour line, `x = 1`.
    SaturatedVapourLine,
    /// Constant-quality lines at `x = 0.1, 0.3, 0.5, 0.7, 0.9`.
    QualityLines,
    /// Selected isobars.
    Isobars,
    /// Selected isotherms.
    Isotherms,
    /// The critical point.
    CriticalPoint,
    /// The triple point.
    TriplePoint,
    /// IAPWS-IF97 region boundaries: the 623.15 K Region 1/3 isotherm, the
    /// Region 2/3 (B23) line, and the 1073.15 K upper isotherm.
    RegionBoundaries,
    /// Wagner / Kretzschmar published saturation-table points.
    WagnerSaturationPoints,
    /// Wagner / Kretzschmar published single-phase isobar-table points.
    WagnerSinglePhasePoints,
    /// Moody (1975) critical-flow stagnation states.
    MoodyStates,
    /// Zaloudek critical-flow throat states.
    ZaloudekStates,
    /// Marviken test 23 and 24 nozzle-inlet stagnation states.
    MarvikenStates,
    /// Edwards–O'Brien initial pipe-node states.
    EdwardsInitialStates,
    /// Edwards–O'Brien measured GS-1 pressure history.
    EdwardsGs1PressureTrace,
}

/// Which specific isobars/isotherms [`LayerId::Isobars`] and
/// [`LayerId::Isotherms`] draw, out of [`curves::DEFAULT_ISOBARS_BAR`] /
/// [`curves::DEFAULT_ISOTHERMS_DEGC`].
///
/// Issue #26 (2026-08-21 follow-up): the isotherm/isobar checkbox used to be
/// all-or-nothing — switching it on drew every default value at once, with no
/// way to pick individual ones. This struct is the subset each layer actually
/// draws; the sidebar's multi-select dropdown edits it, and an empty list for
/// either field means "none of that family," equivalent to the layer being
/// off. [`Default`] draws every default value, matching the tool's prior
/// (only) behaviour, so existing callers that don't care about the selection
/// keep working unchanged.
///
/// Extended 2026-08-21 with `qualities`, the same all-or-nothing-to-selectable
/// fix applied to [`LayerId::QualityLines`] (the fixed `x = 0.1 ... 0.9` set):
/// an empty list means "none of that family," matching `isobars_bar` /
/// `isotherms_degc`.
///
/// Extended again 2026-08-21 with `tabulated_single_phase_axis`,
/// `tabulated_isobars_bar` and `tabulated_isotherms_degc`, per two more
/// maintainer follow-ups on the tabulated single-phase points checkbox:
///
/// * *"I want all the temperatures available from the steam table to choose
///   from in a separate filter under the tabulated data checkbox. do the
///   same for isobars"* -- `tabulated_isobars_bar` / `tabulated_isotherms_degc`
///   are a selection over the tabulated table's own **full** set of values
///   (29 isobars, 98 isotherms as of 2026-08-21), a separate, larger pool
///   than the 10/9-value computed-curve defaults in `isobars_bar` /
///   `isotherms_degc`.
/// * *"make the isotherm isobar toggle available for all diagrams[,] for
///   tabulated data"* -- `tabulated_single_phase_axis` picks isobar or
///   isotherm filtering unconditionally, the same on every diagram (an
///   earlier cut of this forced the axis per-diagram; that restriction is
///   gone).
///
/// See [`wagner_single_phase_states_for_selection`] for exactly how these
/// three fields combine.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerSelection {
    /// Isobars to draw, in bar. Order does not matter.
    pub isobars_bar: Vec<f64>,
    /// Isotherms to draw, in degrees Celsius. Order does not matter.
    pub isotherms_degc: Vec<f64>,
    /// Quality lines to draw, dimensionless (0-1). Order does not matter.
    pub qualities: Vec<f64>,
    /// Which axis filters the tabulated single-phase points layer.
    pub tabulated_single_phase_axis: TabulatedFilterAxis,
    /// Selected isobars, in bar, out of [`tabulated_isobar_values`] -- used
    /// only when `tabulated_single_phase_axis` is [`TabulatedFilterAxis::Isobar`].
    pub tabulated_isobars_bar: Vec<f64>,
    /// Selected isotherms, in degrees Celsius, out of
    /// [`tabulated_isotherm_values`] -- used only when
    /// `tabulated_single_phase_axis` is [`TabulatedFilterAxis::Isotherm`].
    pub tabulated_isotherms_degc: Vec<f64>,
}

/// Which axis quantity filters [`LayerId::WagnerSinglePhasePoints`].
///
/// Available on every diagram (maintainer direction, 2026-08-21: "make the
/// isotherm isobar toggle available for all diagrams[,] for tabulated
/// data"). The matching *computed curve* is still skipped where it would be
/// degenerate on the current diagram (an isobar is a horizontal line on p-h,
/// an isotherm on T-s -- see [`LayerId::availability_on`]), but the
/// tabulated *points* at that value are shown regardless, since a scatter of
/// real data points is not degenerate the way a line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabulatedFilterAxis {
    /// Filter by [`LayerSelection::tabulated_isobars_bar`].
    Isobar,
    /// Filter by [`LayerSelection::tabulated_isotherms_degc`].
    Isotherm,
}

impl Default for LayerSelection {
    fn default() -> Self {
        Self {
            isobars_bar: curves::DEFAULT_ISOBARS_BAR.to_vec(),
            isotherms_degc: curves::DEFAULT_ISOTHERMS_DEGC.to_vec(),
            qualities: curves::QUALITY_LINE_VALUES.to_vec(),
            tabulated_single_phase_axis: TabulatedFilterAxis::Isotherm,
            tabulated_isobars_bar: tabulated_isobar_values(),
            tabulated_isotherms_degc: tabulated_isotherm_values(),
        }
    }
}

/// Every distinct isobar, in bar, the tabulated single-phase table
/// (`reference_data::wagner::WAGNER_SINGLE_PHASE_TABLE`) actually covers,
/// sorted ascending and deduplicated -- 29 values as of 2026-08-21, versus
/// the 10-value `curves::DEFAULT_ISOBARS_BAR` computed-curve default set.
pub fn tabulated_isobar_values() -> Vec<f64> {
    let mut values: Vec<f64> = wagner::WAGNER_SINGLE_PHASE_TABLE
        .iter()
        .map(|row| row[0])
        .collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values.dedup();
    values
}

/// Every distinct isotherm, in degrees Celsius, the tabulated single-phase
/// table covers, sorted ascending and deduplicated -- 98 values as of
/// 2026-08-21, versus the 9-value `curves::DEFAULT_ISOTHERMS_DEGC`
/// computed-curve default set.
pub fn tabulated_isotherm_values() -> Vec<f64> {
    let mut values: Vec<f64> = wagner::WAGNER_SINGLE_PHASE_TABLE
        .iter()
        .map(|row| row[1])
        .collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values.dedup();
    values
}

impl LayerId {
    /// Every layer, in the order they appear in the sidebar.
    pub const ALL: [LayerId; 16] = [
        LayerId::SaturationDome,
        LayerId::SaturatedLiquidLine,
        LayerId::SaturatedVapourLine,
        LayerId::QualityLines,
        LayerId::Isobars,
        LayerId::Isotherms,
        LayerId::CriticalPoint,
        LayerId::TriplePoint,
        LayerId::RegionBoundaries,
        LayerId::WagnerSaturationPoints,
        LayerId::WagnerSinglePhasePoints,
        LayerId::MoodyStates,
        LayerId::ZaloudekStates,
        LayerId::MarvikenStates,
        LayerId::EdwardsInitialStates,
        LayerId::EdwardsGs1PressureTrace,
    ];

    /// Sidebar label.
    pub fn label(self) -> &'static str {
        match self {
            Self::SaturationDome => "Saturation dome",
            Self::SaturatedLiquidLine => "Saturated liquid line (x = 0)",
            Self::SaturatedVapourLine => "Saturated vapour line (x = 1)",
            Self::QualityLines => "Quality lines (x = 0.1 ... 0.9)",
            Self::Isobars => "Isobars",
            Self::Isotherms => "Isotherms",
            Self::CriticalPoint => "Critical point",
            Self::TriplePoint => "Triple point",
            Self::RegionBoundaries => "Region boundaries",
            Self::WagnerSaturationPoints => "Tabulated data: IAPWS saturation table",
            Self::WagnerSinglePhasePoints => "Tabulated data: IAPWS single-phase table",
            Self::MoodyStates => "Moody (1975) critical-flow states",
            Self::ZaloudekStates => "Zaloudek critical-flow throat states",
            Self::MarvikenStates => "Marviken blowdown stagnation states",
            Self::EdwardsInitialStates => "Edwards-O'Brien initial pipe states",
            Self::EdwardsGs1PressureTrace => "Edwards-O'Brien GS-1 pressure trace",
        }
    }

    /// Whether the layer is computed or cited.
    pub fn kind(self) -> LayerKind {
        match self {
            Self::SaturationDome
            | Self::SaturatedLiquidLine
            | Self::SaturatedVapourLine
            | Self::QualityLines
            | Self::Isobars
            | Self::Isotherms
            | Self::CriticalPoint
            | Self::TriplePoint
            | Self::RegionBoundaries => LayerKind::ComputedCurve,
            Self::WagnerSaturationPoints
            | Self::WagnerSinglePhasePoints
            | Self::MoodyStates
            | Self::ZaloudekStates
            | Self::MarvikenStates
            | Self::EdwardsInitialStates
            | Self::EdwardsGs1PressureTrace => LayerKind::ReferencePoints,
        }
    }

    /// Whether the layer starts switched on.
    ///
    /// The structural curves and the markers are on; the reference-data
    /// overlays are off, because switching all six on at once produces an
    /// unreadable figure and the point of the tool is to bring them in one at a
    /// time.
    pub fn default_visible(self) -> bool {
        matches!(
            self,
            Self::SaturationDome
                | Self::QualityLines
                | Self::Isobars
                | Self::Isotherms
                | Self::CriticalPoint
                | Self::TriplePoint
        )
    }

    /// Citation for the layer, carried into the CSV export.
    pub fn provenance(self) -> &'static str {
        match self {
            Self::SaturationDome
            | Self::SaturatedLiquidLine
            | Self::SaturatedVapourLine
            | Self::QualityLines
            | Self::Isobars
            | Self::Isotherms
            | Self::CriticalPoint
            | Self::TriplePoint
            | Self::RegionBoundaries => {
                "computed live from tampines-steam-tables (IAPWS-IF97); not stored data"
            }
            Self::WagnerSaturationPoints | Self::WagnerSinglePhasePoints => {
                WAGNER_KRETZSCHMAR_CITATION
            }
            Self::MoodyStates => {
                "Moody, F. J. (1975), NEDO-21052, Fig. 1 (graph-read); via \
                 moody_critical_mass_flux_homogeneous_eqm.rs"
            }
            Self::ZaloudekStates => {
                "Zaloudek HEM critical-flow curves (graph-read); via \
                 zaloudek_mass_flux_hom_eqm/in_dome_stagnation.rs"
            }
            Self::MarvikenStates => {
                "NUREG/CR-2671 Fig. 8:24 (digitised); via marviken_tests.rs. \
                 Test 23 validated, test 24 NOT validated"
            }
            Self::EdwardsInitialStates => {
                "Tomlinson & Aumiller B-T-3271 Table 1 (Hendrie initial profile); \
                 via tests/edwards_blowdown.rs"
            }
            Self::EdwardsGs1PressureTrace => {
                "Tomlinson & Aumiller B-T-3271 Fig. 3 (digitised); measured PRESSURE \
                 only -- ordinate is the IF97 saturation temperature, not a measurement"
            }
        }
    }

    /// Whether this layer can be drawn on `diagram`, and if not, why not.
    pub fn availability_on(self, diagram: DiagramKind) -> Availability {
        use DiagramKind::{EnthalpyEntropy, PressureEnthalpy, TemperatureEntropy, TemperaturePressure};
        const TP_COLLAPSE: &str =
            "in T-p coordinates the two-phase region collapses onto the vapour-pressure \
             curve, so this would duplicate the saturation dome";
        const FLAT_ISOBAR: &str =
            "an isobar is a horizontal line on this diagram -- it would add no information \
             beyond the pressure grid";
        const FLAT_ISOTHERM: &str =
            "an isotherm is a straight line on this diagram -- it would add no information \
             beyond the temperature grid";
        const PRESSURE_ONLY: &str =
            "only pressure was measured; enthalpy, entropy and quality were not, and this \
             tool will not invent them (issue #26)";

        match (self, diagram) {
            (Self::SaturatedLiquidLine | Self::SaturatedVapourLine, TemperaturePressure) => {
                Availability::Unavailable(TP_COLLAPSE)
            }
            (Self::QualityLines, TemperaturePressure) => Availability::Unavailable(TP_COLLAPSE),
            (Self::Isobars, TemperaturePressure | PressureEnthalpy) => {
                Availability::Unavailable(FLAT_ISOBAR)
            }
            (Self::Isotherms, TemperaturePressure | TemperatureEntropy) => {
                Availability::Unavailable(FLAT_ISOTHERM)
            }
            (
                Self::EdwardsGs1PressureTrace,
                PressureEnthalpy | TemperatureEntropy | EnthalpyEntropy,
            ) => Availability::Unavailable(PRESSURE_ONLY),
            _ => Availability::Available,
        }
    }

    /// The coarser legend bucket this layer belongs to on the live canvas's
    /// Compact legend mode (issue #26: "Group auxiliary curves where
    /// possible: Isotherms / Quality lines / Saturation dome / Validation
    /// points"). The three saturation-envelope layers, which already share a
    /// colour (`INK`), collapse together; every reference-data layer
    /// collapses into one "Validation / reference data" bucket.
    pub fn legend_group(self) -> &'static str {
        match self {
            Self::SaturationDome | Self::SaturatedLiquidLine | Self::SaturatedVapourLine => {
                "Saturation dome"
            }
            Self::QualityLines => "Quality lines",
            Self::Isobars => "Isobars",
            Self::Isotherms => "Isotherms",
            Self::CriticalPoint | Self::TriplePoint => "Critical & triple point",
            Self::RegionBoundaries => "Region boundaries",
            Self::WagnerSaturationPoints
            | Self::WagnerSinglePhasePoints
            | Self::MoodyStates
            | Self::ZaloudekStates
            | Self::MarvikenStates
            | Self::EdwardsInitialStates
            | Self::EdwardsGs1PressureTrace => "Validation / reference data",
        }
    }

    /// Colour used for this layer.
    pub fn colour(self) -> Rgb {
        match self {
            Self::SaturationDome | Self::SaturatedLiquidLine | Self::SaturatedVapourLine => INK,
            Self::QualityLines => PALETTE[8],
            Self::Isobars => PALETTE[0],
            Self::Isotherms => PALETTE[1],
            Self::CriticalPoint | Self::TriplePoint | Self::RegionBoundaries => INK,
            Self::WagnerSaturationPoints => PALETTE[2],
            Self::WagnerSinglePhasePoints => PALETTE[5],
            Self::MoodyStates => PALETTE[3],
            Self::ZaloudekStates => PALETTE[4],
            Self::MarvikenStates => PALETTE[7],
            Self::EdwardsInitialStates => PALETTE[6],
            Self::EdwardsGs1PressureTrace => PALETTE[9],
        }
    }

    /// How this layer is drawn.
    ///
    /// The width tiering follows issue #26's styling table: the saturation
    /// dome is the boldest line on any diagram, deliberately wider than
    /// anything else so it stays dominant against the isotherm/isobar clutter
    /// in both light and dark themes; the saturated-liquid/vapour lines are a
    /// clear middle tier; quality lines, isobars and isotherms are thin
    /// auxiliary curves (further separated from each other by colour and, for
    /// quality lines, a dash).
    pub fn style(self) -> SeriesStyle {
        match self {
            Self::SaturationDome => SeriesStyle::Line {
                width: 2.4,
                dash: None,
            },
            Self::SaturatedLiquidLine | Self::SaturatedVapourLine => SeriesStyle::Line {
                width: 1.4,
                dash: None,
            },
            Self::QualityLines => SeriesStyle::Line {
                width: 0.6,
                dash: Some((3.0, 2.5)),
            },
            Self::Isobars | Self::Isotherms => SeriesStyle::Line {
                width: 0.7,
                dash: None,
            },
            Self::RegionBoundaries => SeriesStyle::Line {
                width: 0.9,
                dash: Some((6.0, 3.0)),
            },
            Self::CriticalPoint => SeriesStyle::Markers {
                shape: MarkerShape::Diamond,
                size: 9.0,
            },
            Self::TriplePoint => SeriesStyle::Markers {
                shape: MarkerShape::Square,
                size: 7.0,
            },
            Self::WagnerSaturationPoints => SeriesStyle::Markers {
                shape: MarkerShape::Plus,
                size: 4.5,
            },
            Self::WagnerSinglePhasePoints => SeriesStyle::Markers {
                shape: MarkerShape::Cross,
                size: 3.5,
            },
            Self::MoodyStates => SeriesStyle::Markers {
                shape: MarkerShape::Circle,
                size: 4.5,
            },
            Self::ZaloudekStates => SeriesStyle::Markers {
                shape: MarkerShape::Triangle,
                size: 4.5,
            },
            Self::MarvikenStates => SeriesStyle::Markers {
                shape: MarkerShape::Square,
                size: 5.0,
            },
            Self::EdwardsInitialStates => SeriesStyle::Markers {
                shape: MarkerShape::OpenCircle,
                size: 6.0,
            },
            Self::EdwardsGs1PressureTrace => SeriesStyle::Markers {
                shape: MarkerShape::Diamond,
                size: 5.0,
            },
        }
    }

    /// Builds this layer's geometry.
    ///
    /// Returns an empty vector when the layer is unavailable on `diagram` — the
    /// caller has already been told why by [`LayerId::availability_on`].
    ///
    /// `curve_samples` controls how finely the computed curves are sampled; it
    /// is the GUI's resolution slider.
    pub fn build(
        self,
        diagram: DiagramKind,
        curve_samples: usize,
        selection: &LayerSelection,
    ) -> Vec<PlotLayer> {
        if !self.availability_on(diagram).is_available() {
            return Vec::new();
        }
        let base_coloured =
            |label: String, segments: Vec<Vec<ThermoPoint>>, legend: bool, colour: Rgb| PlotLayer {
                label,
                kind: self.kind(),
                provenance: self.provenance().to_string(),
                segments,
                style: self.style(),
                colour,
                show_in_legend: legend,
                legend_group: self.legend_group(),
                custom_line: None,
            };
        let base = |label: String, segments: Vec<Vec<ThermoPoint>>, legend: bool| {
            base_coloured(label, segments, legend, self.colour())
        };

        match self {
            Self::SaturationDome => {
                let (liquid, vapour) = curves::saturation_lines(curve_samples);
                // One closed outline: up the liquid line, back down the vapour
                // line. Reversing the vapour branch is what makes it a dome
                // rather than two disconnected arcs.
                let mut outline = liquid;
                outline.extend(vapour.into_iter().rev());
                vec![base("Saturation dome".to_string(), vec![outline], true)]
            }
            Self::SaturatedLiquidLine => {
                let (liquid, _) = curves::saturation_lines(curve_samples);
                vec![base(
                    "Saturated liquid (x = 0)".to_string(),
                    vec![liquid],
                    true,
                )]
            }
            Self::SaturatedVapourLine => {
                let (_, vapour) = curves::saturation_lines(curve_samples);
                vec![base(
                    "Saturated vapour (x = 1)".to_string(),
                    vec![vapour],
                    true,
                )]
            }
            Self::QualityLines => selection
                .qualities
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    base(
                        format!("Quality x = {x}"),
                        vec![curves::quality_line(*x, curve_samples)],
                        i == 0,
                    )
                })
                .collect(),
            // Each isobar's computed line is paired with the published Wagner
            // rows at (close to) that same pressure, if any exist — issue #26:
            // "plot the tabulated data as crosses and iapws97 data as lines...
            // so that a csv plot and graphical plot can be obtained". A default
            // isobar Wagner never tabulated (see `TabulatedData::isobar`'s
            // doc) contributes no cross layer, which is itself the honest
            // answer: this crate has no published check at that pressure.
            Self::Isobars => selection
                .isobars_bar
                .iter()
                .enumerate()
                .flat_map(|(i, p_bar)| {
                    let line = isobar_computed_line(*p_bar, curve_samples, i == 0);
                    let crosses = tabulated_cross_layer(
                        format!("Isobar p = {p_bar} bar (tabulated)"),
                        TabulatedData.isobar_default_tolerance(Pressure::new::<bar>(*p_bar)),
                        self.colour(),
                        i == 0,
                        self.legend_group(),
                    );
                    std::iter::once(line).chain(crosses)
                })
                .collect(),
            // Coloured by temperature (issue #26 "Option A: colour gradient"),
            // not the flat `self.colour()` every other layer uses — otherwise
            // every isotherm looks identical and the set is unreadable once
            // more than two or three are switched on. Wagner crosses follow
            // suit so a cross and its own line share a colour; see `Isobars`
            // above for why a match may be empty.
            Self::Isotherms => selection
                .isotherms_degc
                .iter()
                .enumerate()
                .flat_map(|(i, t_degc)| {
                    let colour = crate::figure::isotherm_colour(*t_degc);
                    let line = isotherm_computed_line(*t_degc, curve_samples, i == 0);
                    let crosses = tabulated_cross_layer(
                        format!("Isotherm T = {t_degc} \u{00B0}C (tabulated)"),
                        TabulatedData.isotherm_default_tolerance(ThermodynamicTemperature::new::<
                            degree_celsius,
                        >(*t_degc)),
                        colour,
                        i == 0,
                        self.legend_group(),
                    );
                    std::iter::once(line).chain(crosses)
                })
                .collect(),
            Self::CriticalPoint => vec![base(
                "Critical point (647.096 K, 22.064 MPa)".to_string(),
                vec![vec![curves::critical_point()]],
                true,
            )],
            Self::TriplePoint => vec![base(
                "Triple point (273.16 K, 611.657 Pa)".to_string(),
                vec![vec![curves::triple_point_liquid()]],
                true,
            )],
            Self::RegionBoundaries => vec![base(
                "IF97 region boundaries".to_string(),
                region_boundary_segments(curve_samples),
                true,
            )],
            Self::WagnerSaturationPoints => {
                let (liquid, vapour) = wagner_saturation_states();
                vec![
                    base(
                        "Tabulated saturated liquid (table)".to_string(),
                        vec![liquid],
                        true,
                    ),
                    base(
                        "Tabulated saturated vapour (table)".to_string(),
                        vec![vapour],
                        false,
                    ),
                ]
            }
            // Filtered by exactly one axis -- `selection.tabulated_single_phase_axis`
            // -- out of the *full* set of isobars/isotherms the tabulated
            // table actually covers (not the 10/9-value computed-curve
            // defaults), per the maintainer's 2026-08-21 follow-ups: "I want
            // all the temperatures available from the steam table to choose
            // from in a separate filter under the tabulated data checkbox.
            // do the same for isobars" and "make the isotherm isobar toggle
            // available for all diagrams[,] for tabulated data" -- the axis
            // choice is no longer diagram-forced.
            //
            // Also plots the matching computed IAPWS-IF97 line for every
            // selected value ("please plot the iapws smooth curve along with
            // it"), skipped only where that curve would be degenerate on
            // this diagram (see [`LayerId::availability_on`]) -- a flat line
            // duplicating the axis grid is not useful even though the
            // tabulated *points* at that value still are.
            Self::WagnerSinglePhasePoints => {
                let mut layers: Vec<PlotLayer> = match selection.tabulated_single_phase_axis {
                    TabulatedFilterAxis::Isobar
                        if Self::Isobars.availability_on(diagram).is_available() =>
                    {
                        selection
                            .tabulated_isobars_bar
                            .iter()
                            .enumerate()
                            .map(|(i, p_bar)| isobar_computed_line(*p_bar, curve_samples, i == 0))
                            .collect()
                    }
                    TabulatedFilterAxis::Isotherm
                        if Self::Isotherms.availability_on(diagram).is_available() =>
                    {
                        selection
                            .tabulated_isotherms_degc
                            .iter()
                            .enumerate()
                            .map(|(i, t_degc)| {
                                isotherm_computed_line(*t_degc, curve_samples, i == 0)
                            })
                            .collect()
                    }
                    TabulatedFilterAxis::Isobar | TabulatedFilterAxis::Isotherm => Vec::new(),
                };
                layers.push(base(
                    "Tabulated single-phase table points".to_string(),
                    vec![wagner_single_phase_states_for_selection(selection)],
                    true,
                ));
                layers
            }
            Self::MoodyStates => vec![base(
                "Moody (1975) stagnation states".to_string(),
                vec![moody_states()],
                true,
            )],
            Self::ZaloudekStates => vec![base(
                "Zaloudek throat states".to_string(),
                vec![zaloudek_states()],
                true,
            )],
            Self::MarvikenStates => marviken::MARVIKEN_TESTS
                .iter()
                .enumerate()
                .map(|(i, test)| base(test.label.to_string(), vec![marviken_states(test)], i == 0))
                .collect(),
            Self::EdwardsInitialStates => vec![base(
                "Edwards-O'Brien initial node states".to_string(),
                vec![edwards_initial_states()],
                true,
            )],
            Self::EdwardsGs1PressureTrace => vec![base(
                "Edwards-O'Brien GS-1 measured pressure".to_string(),
                vec![edwards_gs1_states()],
                true,
            )],
        }
    }
}

/// The three IAPWS-IF97 region boundaries worth drawing.
///
/// * the 623.15 K isotherm, which separates Region 1 from Region 3,
/// * the Region 2 / Region 3 (B23) line, `p_B23(T)` from 623.15 K to 863.15 K,
/// * the 1073.15 K isotherm, above which Region 5 takes over and the backward
///   `(p,h)` equations stop existing.
fn region_boundary_segments(samples: usize) -> Vec<Vec<ThermoPoint>> {
    let mut segments = Vec::new();

    for t_kelvin in [curves::T_REGION_13_BOUNDARY_KELVIN, curves::T_MAX_KELVIN] {
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
        let mut points = Vec::new();
        for segment in curves::isotherm(t, samples) {
            points.extend(segment);
        }
        if points.len() >= 2 {
            segments.push(points);
        }
    }

    let n = samples.max(2);
    let b23: Vec<ThermoPoint> = (0..n)
        .filter_map(|i| {
            let t_kelvin = 623.15 + (863.15 - 623.15) * i as f64 / (n - 1) as f64;
            let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
            curves::single_phase_point(t, p_boundary_2_3(t))
        })
        .collect();
    if b23.len() >= 2 {
        segments.push(b23);
    }
    segments
}

/// Wagner saturation-table rows as plotted states: the tabulated saturated
/// liquid and saturated vapour points.
///
/// Every coordinate is taken straight from the published table — pressure,
/// temperature, enthalpy and entropy alike. Nothing is recomputed, which is the
/// point: these are the reference the computed dome is judged against.
fn wagner_saturation_states() -> (Vec<ThermoPoint>, Vec<ThermoPoint>) {
    let mut liquid = Vec::with_capacity(wagner::WAGNER_SATURATION_TABLE.len());
    let mut vapour = Vec::with_capacity(wagner::WAGNER_SATURATION_TABLE.len());
    for row in wagner::WAGNER_SATURATION_TABLE {
        let t = ThermodynamicTemperature::new::<degree_celsius>(row[wagner::SAT_COL_T_DEGC]);
        let p = Pressure::new::<bar>(row[wagner::SAT_COL_P_BAR]);
        liquid.push(ThermoPoint::new(
            p,
            t,
            AvailableEnergy::new::<kilojoule_per_kilogram>(row[wagner::SAT_COL_H_LIQ]),
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(row[wagner::SAT_COL_S_LIQ]),
            Some(0.0),
        ));
        vapour.push(ThermoPoint::new(
            p,
            t,
            AvailableEnergy::new::<kilojoule_per_kilogram>(row[wagner::SAT_COL_H_VAP]),
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(row[wagner::SAT_COL_S_VAP]),
            Some(1.0),
        ));
    }
    (liquid, vapour)
}

/// The computed isobar line at `p_bar`, styled and labelled exactly as
/// [`LayerId::Isobars`] draws it. Shared by [`LayerId::Isobars`] itself and
/// [`LayerId::WagnerSinglePhasePoints`] (issue #26, 2026-08-21: "please plot
/// the iapws smooth curve along with it" whenever the tabulated-data
/// checkbox is filtering by isobar), so the two can never draw the same
/// isobar differently.
fn isobar_computed_line(p_bar: f64, curve_samples: usize, show_in_legend: bool) -> PlotLayer {
    let p = Pressure::new::<bar>(p_bar);
    PlotLayer {
        label: format!("Isobar p = {p_bar} bar"),
        kind: LayerId::Isobars.kind(),
        provenance: LayerId::Isobars.provenance().to_string(),
        segments: curves::isobar(p, curve_samples),
        style: LayerId::Isobars.style(),
        colour: LayerId::Isobars.colour(),
        show_in_legend,
        legend_group: LayerId::Isobars.legend_group(),
        custom_line: None,
    }
}

/// The computed isotherm line at `t_degc`, styled and labelled exactly as
/// [`LayerId::Isotherms`] draws it (including its temperature-graded
/// colour). Shared the same way [`isobar_computed_line`] is.
fn isotherm_computed_line(t_degc: f64, curve_samples: usize, show_in_legend: bool) -> PlotLayer {
    let t = ThermodynamicTemperature::new::<degree_celsius>(t_degc);
    PlotLayer {
        label: format!("Isotherm T = {t_degc} \u{00B0}C"),
        kind: LayerId::Isotherms.kind(),
        provenance: LayerId::Isotherms.provenance().to_string(),
        segments: curves::isotherm(t, curve_samples),
        style: LayerId::Isotherms.style(),
        colour: crate::figure::isotherm_colour(t_degc),
        show_in_legend,
        legend_group: LayerId::Isotherms.legend_group(),
        custom_line: None,
    }
}

/// Builds the "Wagner crosses" companion layer for one isobar/isotherm line
/// (issue #26: "plot the tabulated data as crosses and iapws97 data as
/// lines"), via [`TabulatedData`]. `None` when the query matched no published
/// row — the caller's default isobar/isotherm sweep is a *visual coverage*
/// set, not a claim that Wagner tabulated every value in it, so an empty
/// match is the correct, honest outcome, not an error.
fn tabulated_cross_layer(
    label: String,
    states: Vec<TabulatedState>,
    colour: Rgb,
    show_in_legend: bool,
    legend_group: &'static str,
) -> Option<PlotLayer> {
    if states.is_empty() {
        return None;
    }
    let points: Vec<ThermoPoint> = states
        .into_iter()
        .map(|s| {
            ThermoPoint::new(
                s.pressure,
                s.temperature,
                s.specific_enthalpy,
                s.specific_entropy,
                None,
            )
        })
        .collect();
    Some(PlotLayer {
        label,
        kind: LayerKind::ReferencePoints,
        provenance: format!(
            "{WAGNER_KRETZSCHMAR_CITATION} -- published tabulated value(s) matching this line, \
             not this crate's own computation"
        ),
        segments: vec![points],
        style: SeriesStyle::Markers {
            shape: MarkerShape::Cross,
            size: 3.5,
        },
        colour,
        show_in_legend,
        legend_group,
        custom_line: None,
    })
}

/// The citation for the tabulated single-phase and saturation-table data
/// (issue #26, 2026-08-21: "ensure to include the citation within the gui
/// app"), as the maintainer supplied it verbatim. Used everywhere this GUI
/// cites that table, so the reference cannot drift between call sites --
/// see [`crate::citations`] for where it is surfaced to the user.
pub(crate) const WAGNER_KRETZSCHMAR_CITATION: &str =
    "Wagner, W., & Kretzschmar, H. J. (2008). International steam tables: Properties of water \
     and steam based on the industrial formulation IAPWS-IF97. Berlin, Heidelberg: Springer \
     Berlin Heidelberg.";

/// Wagner single-phase table rows as plotted states, restricted to
/// `selection.tabulated_single_phase_axis` and the matching
/// `tabulated_isobars_bar` / `tabulated_isotherms_degc` list -- **one** axis,
/// never a union, and the same axis on every diagram (maintainer direction,
/// 2026-08-21: *"make the isotherm isobar toggle available for all
/// diagrams[,] for tabulated data"* -- an earlier cut forced the axis by
/// diagram; that is gone).
///
/// Queries [`TabulatedData::isobar_default_tolerance`] or
/// [`TabulatedData::isotherm_default_tolerance`] for every selected value on
/// the chosen axis. An empty selection on that axis yields no points -- the
/// honest "nothing selected" outcome, not a fallback to the full table. See
/// [`LayerId::WagnerSinglePhasePoints`]'s `build` arm for how the matching
/// computed IAPWS-IF97 line is paired in alongside these points.
fn wagner_single_phase_states_for_selection(selection: &LayerSelection) -> Vec<ThermoPoint> {
    let mut states: Vec<TabulatedState> = Vec::new();
    match selection.tabulated_single_phase_axis {
        TabulatedFilterAxis::Isobar => {
            for p_bar in &selection.tabulated_isobars_bar {
                states.extend(TabulatedData.isobar_default_tolerance(Pressure::new::<bar>(*p_bar)));
            }
        }
        TabulatedFilterAxis::Isotherm => {
            for t_degc in &selection.tabulated_isotherms_degc {
                states.extend(TabulatedData.isotherm_default_tolerance(
                    ThermodynamicTemperature::new::<degree_celsius>(*t_degc),
                ));
            }
        }
    }
    states.sort_by(|a, b| {
        a.pressure
            .get::<pascal>()
            .partial_cmp(&b.pressure.get::<pascal>())
            .unwrap()
            .then(
                a.temperature
                    .get::<kelvin>()
                    .partial_cmp(&b.temperature.get::<kelvin>())
                    .unwrap(),
            )
    });
    states.dedup();
    states
        .into_iter()
        .map(|s| {
            ThermoPoint::new(
                s.pressure,
                s.temperature,
                s.specific_enthalpy,
                s.specific_entropy,
                None,
            )
        })
        .collect()
}

/// Moody stagnation states.
///
/// # What is data and what is computed
///
/// `p0` and `h0` are Moody's own, recovered by multiplying the dimensionless
/// chart values by his reference scales. Temperature and entropy are **not** in
/// the chart, so they come from this crate's `(p,h)` flash — which is legitimate
/// because `(p, h)` already fixes the state completely; the flash is a change of
/// coordinates, not new information. A point whose flash this crate declines
/// (Region 5, or outside the `(p,h)` envelope) is dropped rather than clamped.
fn moody_states() -> Vec<ThermoPoint> {
    let mut points = Vec::new();
    for isobar in moody::MOODY_ISOBARS {
        let p0 = Pressure::new::<pound_force_per_square_inch>(
            isobar.p0_over_p_ref * moody::MOODY_P_REF_PSI,
        );
        for (h_ratio, g_ratio) in isobar.points {
            let h0 = AvailableEnergy::new::<btu_it_per_pound>(
                h_ratio * moody::MOODY_H_REF_BTU_IT_PER_LB,
            );
            let Some(point) = point_from_ph(p0, h0) else {
                continue;
            };
            let g =
                MassRate::new::<pound_per_second>(g_ratio * moody::MOODY_G_REF_LB_PER_S_PER_FT2)
                    / Area::new::<square_foot>(1.0);
            points.push(point.with_reference_mass_flux(g));
        }
    }
    points
}

/// Zaloudek throat states.
///
/// # What is data and what is computed
///
/// The critical (throat) pressure and the throat quality `x_t` are Zaloudek's.
/// The state is then the Region-4 saturated mixture at that pressure and
/// quality, with `h` and `s` from the lever rule on this crate's saturated
/// properties — the same lever rule issue #26 specifies, and the same one the
/// crate's own test fixture uses to build the throat state.
fn zaloudek_states() -> Vec<ThermoPoint> {
    let mut points = Vec::new();
    for curve in zaloudek::ZALOUDEK_CURVES {
        for (p_psia, g_lb_per_s_ft2, _h0_btu) in curve.points {
            let p = Pressure::new::<pound_force_per_square_inch>(*p_psia);
            let t_sat = sat_temp_4(p);
            let Some(state) = curves::saturation_state(t_sat) else {
                continue;
            };
            let g =
                MassRate::new::<pound_per_second>(*g_lb_per_s_ft2) / Area::new::<square_foot>(1.0);
            let point = state.at_quality(curve.throat_quality);
            if point.is_finite() {
                points.push(point.with_reference_mass_flux(g));
            }
        }
    }
    points
}

/// Marviken nozzle-inlet stagnation states.
///
/// # What is data and what is computed
///
/// The stagnation **pressure** and the **mass flux** are the digitised
/// measurement. The stagnation **enthalpy is not measured**; it is
/// reconstructed exactly as `marviken_tests.rs` reconstructs it — subcooled
/// liquid at the vessel water temperature while the vessel is still subcooled,
/// saturated liquid once it has flashed — so the plotted state matches the state
/// the crate's own V&V feeds to its solver, rather than a second, differing
/// reconstruction.
fn marviken_states(test: &marviken::MarvikenTest) -> Vec<ThermoPoint> {
    let t_water = ThermodynamicTemperature::new::<degree_celsius>(test.water_temperature_degc);
    let mut points = Vec::new();
    for (p_kpa, g_kg_per_m2_s) in test.points {
        let p = Pressure::new::<kilopascal>(*p_kpa);
        let t_sat = sat_temp_4(p);
        let t_state = if t_sat.get::<kelvin>() > t_water.get::<kelvin>() {
            t_water
        } else {
            t_sat
        };
        // Both branches are saturated or subcooled liquid, so Region 1 is the
        // right equation set in each case.
        let point = ThermoPoint::new(
            p,
            t_state,
            h_tp_1(t_state, p),
            s_tp_1(t_state, p),
            (t_state.get::<kelvin>() >= t_sat.get::<kelvin>()).then_some(0.0),
        );
        if point.is_finite() {
            points.push(point.with_reference_mass_flux(MassFlux::new::<
                kilogram_per_square_meter_second,
            >(*g_kg_per_m2_s)));
        }
    }
    points
}

/// Edwards–O'Brien initial pipe-node states: 24 measured node temperatures at
/// the measured 7.0 MPa initial pressure.
fn edwards_initial_states() -> Vec<ThermoPoint> {
    let p = Pressure::new::<pascal>(edwards::EDWARDS_P_INIT_PA);
    edwards::EDWARDS_NODE_T_DEGF
        .iter()
        .filter_map(|t_degf| {
            let t = ThermodynamicTemperature::new::<degree_fahrenheit>(*t_degf);
            curves::single_phase_point(t, p)
        })
        .collect()
}

/// Edwards–O'Brien GS-1 measured pressure history, T-p tab only.
///
/// # Honest labelling
///
/// The abscissa on the T-p diagram is the IAPWS-IF97 **saturation temperature**
/// at each measured pressure, not a measured temperature — the experiment
/// reported no temperature at GS-1. Placing the points on the vapour-pressure
/// curve is a statement about where a flashing blowdown is expected to sit, and
/// the figure's footnote and the layer's provenance string both say so. The
/// layer is unavailable on every diagram that needs an enthalpy or an entropy,
/// because neither was measured.
fn edwards_gs1_states() -> Vec<ThermoPoint> {
    edwards::EDWARDS_GS1_DATA_PSIA
        .iter()
        .filter_map(|(_t_seconds, p_psia)| {
            let p = Pressure::new::<pound_force_per_square_inch>(*p_psia);
            let t_sat = sat_temp_4(p);
            let state = curves::saturation_state(t_sat)?;
            let point = state.at_quality(0.0);
            point.is_finite().then_some(point)
        })
        .collect()
}

/// Completes a state from `(p, h)` through this crate's checked backward
/// equations, returning `None` where the crate declines.
fn point_from_ph(p: Pressure, h: AvailableEnergy) -> Option<ThermoPoint> {
    let t = try_t_ph_eqm(p, h).ok()?;
    let s = try_s_ph_eqm(p, h).ok()?;
    let quality = try_x_ph_flash(p, h).ok();
    // Quality is only meaningful inside the dome; outside it the flash reports a
    // saturating 0 or 1, which would be misleading in a CSV column.
    let inside_dome = (p.get::<pascal>() - sat_pressure_4(t).get::<pascal>()).abs()
        <= sat_pressure_4(t).get::<pascal>() * 1.0e-6;
    let point = ThermoPoint::new(p, t, h, s, inside_dome.then_some(quality?));
    point.is_finite().then_some(point)
}

/// Checks the availability table against the two rules it encodes.
///
/// # Methodology
///
/// Walks all 16 layers on all 4 diagrams. Asserts that (a) every unavailable
/// entry carries a non-empty reason, (b) the layers that must be available
/// everywhere — the saturation dome, both point markers, and every
/// full-state reference dataset — are, and (c) the specific degenerate and
/// missing-data cases are the ones actually disabled: quality lines and the two
/// saturation branch lines off on T-p, isobars off on T-p and p-h, isotherms
/// off on T-p and T-s, and the Edwards GS-1 trace on **only** on T-p.
///
/// # Result (measured 2026-08-20)
///
/// Passes: 54 of the 64 layer-diagram pairs are available. The 10 unavailable
/// ones are exactly the cases listed above, each with a reason string: the two
/// saturation branch lines and the quality lines on T-p (3), isobars on T-p and
/// p-h (2), isotherms on T-p and T-s (2), and the Edwards GS-1 trace on p-h,
/// T-s and h-s (3).
#[cfg(test)]
#[test]
fn availability_table_disables_only_degenerate_or_unmeasured_cases() {
    use DiagramKind::{EnthalpyEntropy, PressureEnthalpy, TemperatureEntropy, TemperaturePressure};

    let mut unavailable = 0usize;
    for layer in LayerId::ALL {
        for diagram in DiagramKind::ALL {
            match layer.availability_on(diagram) {
                Availability::Available => {}
                Availability::Unavailable(reason) => {
                    assert!(!reason.is_empty(), "{layer:?} on {diagram:?} has no reason");
                    unavailable += 1;
                }
            }
        }
    }
    assert_eq!(unavailable, 10, "the disabled set changed unexpectedly");

    for layer in [
        LayerId::SaturationDome,
        LayerId::CriticalPoint,
        LayerId::TriplePoint,
        LayerId::WagnerSaturationPoints,
        LayerId::WagnerSinglePhasePoints,
        LayerId::MoodyStates,
        LayerId::ZaloudekStates,
        LayerId::MarvikenStates,
        LayerId::EdwardsInitialStates,
    ] {
        for diagram in DiagramKind::ALL {
            assert!(
                layer.availability_on(diagram).is_available(),
                "{layer:?} should be available on {diagram:?}"
            );
        }
    }

    assert!(!LayerId::QualityLines
        .availability_on(TemperaturePressure)
        .is_available());
    assert!(!LayerId::Isobars
        .availability_on(PressureEnthalpy)
        .is_available());
    assert!(LayerId::Isobars
        .availability_on(TemperatureEntropy)
        .is_available());
    assert!(!LayerId::Isotherms
        .availability_on(TemperatureEntropy)
        .is_available());
    assert!(LayerId::Isotherms
        .availability_on(PressureEnthalpy)
        .is_available());
    assert!(LayerId::EdwardsGs1PressureTrace
        .availability_on(TemperaturePressure)
        .is_available());
    assert!(!LayerId::EdwardsGs1PressureTrace
        .availability_on(EnthalpyEntropy)
        .is_available());
}

/// Checks that every reference-data layer actually produces points.
///
/// # Methodology
///
/// Builds every layer on the h-s diagram (which needs a full four-variable
/// state, so it is the strictest of the four) at a low sample count, and asserts
/// each reference-data layer yields a non-zero point count. A dataset that
/// silently produced nothing — because every point fell outside the flash
/// envelope, say — would otherwise look like a working but empty layer, which is
/// exactly the failure mode the "disable, do not fabricate" rule is meant to
/// make visible.
///
/// # Result (measured 2026-08-20)
///
/// Passes. Point counts on h-s: Wagner saturation 220 + 220, Wagner
/// single-phase 2334, Moody 321 minus any states outside the `(p,h)` envelope,
/// Zaloudek 357, Marviken 29 + 40, Edwards initial 24.
#[cfg(test)]
#[test]
fn every_reference_layer_yields_points() {
    for layer in LayerId::ALL {
        if layer.kind() != LayerKind::ReferencePoints {
            continue;
        }
        let diagram = if layer == LayerId::EdwardsGs1PressureTrace {
            DiagramKind::TemperaturePressure
        } else {
            DiagramKind::EnthalpyEntropy
        };
        let built = layer.build(diagram, 60, &LayerSelection::default());
        let total: usize = built.iter().map(PlotLayer::point_count).sum();
        assert!(total > 0, "{layer:?} produced no points on {diagram:?}");
    }
}

/// Checks [`LayerId::WagnerSinglePhasePoints`]'s reference-points sub-layer
/// respects the tabulated isobar/isotherm multi-select rather than always
/// dumping the full 2 334-row table (issue #26, 2026-08-21).
///
/// # Methodology
///
/// Builds the layer three ways: the default selection (every tabulated
/// isotherm, [`LayerSelection::default`]'s axis), a selection naming only the
/// 100 bar isobar (a value known to be tabulated) with the axis set to
/// [`TabulatedFilterAxis::Isobar`], and an empty isobar selection on the same
/// axis. Only the `ReferencePoints`-kind sub-layer is summed for the point
/// counts -- the layer also carries `ComputedCurve` line(s) since
/// 2026-08-21 (see [`tabulated_single_phase_pairs_a_smooth_curve_where_not_degenerate`]),
/// which must not be counted as tabulated points.
///
/// # Result (measured 2026-08-21)
///
/// Holds: default selection matches [`wagner_single_phase_states_for_selection`]
/// called directly; 100-bar-only yields exactly its own rows, all at 100 bar;
/// empty yields nothing.
#[cfg(test)]
#[test]
fn wagner_single_phase_points_respect_the_isobar_isotherm_selection() {
    let diagram = DiagramKind::EnthalpyEntropy;
    let reference_point_count = |built: &[PlotLayer]| -> usize {
        built
            .iter()
            .filter(|l| l.kind == LayerKind::ReferencePoints)
            .map(PlotLayer::point_count)
            .sum()
    };

    let default_selection = LayerSelection::default();
    let default_built = LayerId::WagnerSinglePhasePoints.build(diagram, 20, &default_selection);
    let default_count = reference_point_count(&default_built);
    let direct_count = wagner_single_phase_states_for_selection(&default_selection).len();
    assert_eq!(
        default_count, direct_count,
        "the built layer's reference-point count must match \
         wagner_single_phase_states_for_selection exactly, or the two have drifted apart"
    );

    let hundred_bar_only = LayerSelection {
        tabulated_single_phase_axis: TabulatedFilterAxis::Isobar,
        tabulated_isobars_bar: vec![100.0],
        tabulated_isotherms_degc: Vec::new(),
        ..LayerSelection::default()
    };
    let states = TabulatedData.isobar_default_tolerance(Pressure::new::<bar>(100.0));
    assert!(
        !states.is_empty(),
        "100 bar must be a tabulated Wagner isobar for this test to mean anything"
    );
    let built = LayerId::WagnerSinglePhasePoints.build(diagram, 20, &hundred_bar_only);
    let count = reference_point_count(&built);
    assert_eq!(
        count,
        states.len(),
        "selecting only the 100 bar isobar must draw exactly its own rows"
    );
    assert!(
        count < default_count,
        "a single-isobar selection must yield strictly fewer points than the default selection"
    );
    for layer in built
        .iter()
        .filter(|l| l.kind == LayerKind::ReferencePoints)
    {
        for segment in &layer.segments {
            for point in segment {
                let p_bar = point.pressure.get::<bar>();
                assert!(
                    (p_bar - 100.0).abs() < 1.0e-6,
                    "expected every point at 100 bar, got {p_bar} bar"
                );
            }
        }
    }

    let empty = LayerSelection {
        tabulated_single_phase_axis: TabulatedFilterAxis::Isobar,
        tabulated_isobars_bar: Vec::new(),
        tabulated_isotherms_degc: Vec::new(),
        ..LayerSelection::default()
    };
    let built_empty = LayerId::WagnerSinglePhasePoints.build(diagram, 20, &empty);
    let empty_count = reference_point_count(&built_empty);
    assert_eq!(
        empty_count, 0,
        "an empty isobar/isotherm selection must draw no tabulated single-phase points"
    );
}

/// Checks the tabulated single-phase axis toggle behaves identically on
/// every diagram (maintainer direction, 2026-08-21: *"make the isotherm
/// isobar toggle available for all diagrams[,] for tabulated data"* -- an
/// earlier cut forced the axis per diagram; that restriction is gone).
///
/// # Methodology
///
/// Builds a selection naming a real isobar (100 bar) and a real isotherm
/// (300 degC), then on each of the four diagrams builds the layer once with
/// the axis set to Isobar and once set to Isotherm, checking the
/// reference-point count matches the isobar-only / isotherm-only row count
/// in both cases -- i.e. the choice is respected the same way regardless of
/// which diagram is showing.
///
/// # Result (measured 2026-08-21)
///
/// Holds on all four diagrams.
#[cfg(test)]
#[test]
fn tabulated_single_phase_axis_toggle_works_on_every_diagram() {
    let p_bar = 100.0;
    let t_degc = 300.0;
    let isobar_only_count = TabulatedData
        .isobar_default_tolerance(Pressure::new::<bar>(p_bar))
        .len();
    let isotherm_only_count = TabulatedData
        .isotherm_default_tolerance(ThermodynamicTemperature::new::<degree_celsius>(t_degc))
        .len();
    assert!(isobar_only_count > 0 && isotherm_only_count > 0);
    assert_ne!(
        isobar_only_count, isotherm_only_count,
        "the two counts must differ or this test cannot tell which axis was actually used"
    );

    let reference_point_count = |built: &[PlotLayer]| -> usize {
        built
            .iter()
            .filter(|l| l.kind == LayerKind::ReferencePoints)
            .map(PlotLayer::point_count)
            .sum()
    };

    for diagram in DiagramKind::ALL {
        let isobar_selection = LayerSelection {
            tabulated_single_phase_axis: TabulatedFilterAxis::Isobar,
            tabulated_isobars_bar: vec![p_bar],
            tabulated_isotherms_degc: vec![t_degc],
            ..LayerSelection::default()
        };
        let isobar_count = reference_point_count(&LayerId::WagnerSinglePhasePoints.build(
            diagram,
            20,
            &isobar_selection,
        ));
        assert_eq!(
            isobar_count, isobar_only_count,
            "{diagram:?}: axis = Isobar must filter by isobar, ignoring the isotherm selection"
        );

        let isotherm_selection = LayerSelection {
            tabulated_single_phase_axis: TabulatedFilterAxis::Isotherm,
            ..isobar_selection
        };
        let isotherm_count = reference_point_count(&LayerId::WagnerSinglePhasePoints.build(
            diagram,
            20,
            &isotherm_selection,
        ));
        assert_eq!(
            isotherm_count, isotherm_only_count,
            "{diagram:?}: axis = Isotherm must filter by isotherm, ignoring the isobar selection"
        );
    }
}

/// Checks the tabulated single-phase layer pairs in the matching computed
/// IAPWS-IF97 line wherever that curve is not degenerate on the current
/// diagram, and omits it (while still drawing the tabulated points) where it
/// would be (issue #26, 2026-08-21: *"please plot the iapws smooth curve
/// along with it"*).
///
/// # Methodology
///
/// h-s draws both isobars and isotherms (neither is degenerate there), so a
/// 100-bar isobar selection there must include a `ComputedCurve` layer whose
/// label names "100 bar". p-h cannot draw isobars (a horizontal line) — an
/// isobar-axis selection there must still produce the tabulated
/// `ReferencePoints`, but no `ComputedCurve` layer. T-s cannot draw isotherms
/// — an isotherm-axis selection there must likewise keep the points and drop
/// the curve.
///
/// # Result (measured 2026-08-21)
///
/// Holds: h-s pairs the curve in; p-h and T-s omit it while keeping the
/// tabulated points.
#[cfg(test)]
#[test]
fn tabulated_single_phase_pairs_a_smooth_curve_where_not_degenerate() {
    let isobar_selection = LayerSelection {
        tabulated_single_phase_axis: TabulatedFilterAxis::Isobar,
        tabulated_isobars_bar: vec![100.0],
        tabulated_isotherms_degc: Vec::new(),
        ..LayerSelection::default()
    };
    let hs_built =
        LayerId::WagnerSinglePhasePoints.build(DiagramKind::EnthalpyEntropy, 20, &isobar_selection);
    assert!(
        hs_built
            .iter()
            .any(|l| l.kind == LayerKind::ComputedCurve && l.label.contains("100 bar")),
        "h-s must pair in the computed 100 bar isobar line"
    );
    assert!(
        hs_built
            .iter()
            .any(|l| l.kind == LayerKind::ReferencePoints),
        "h-s must still draw the tabulated points"
    );

    let ph_built = LayerId::WagnerSinglePhasePoints.build(
        DiagramKind::PressureEnthalpy,
        20,
        &isobar_selection,
    );
    assert!(
        !ph_built.iter().any(|l| l.kind == LayerKind::ComputedCurve),
        "p-h must not pair in an isobar line -- it is a horizontal line there"
    );
    assert!(
        ph_built
            .iter()
            .any(|l| l.kind == LayerKind::ReferencePoints),
        "p-h must still draw the tabulated points even without the curve"
    );

    let isotherm_selection = LayerSelection {
        tabulated_single_phase_axis: TabulatedFilterAxis::Isotherm,
        tabulated_isobars_bar: Vec::new(),
        tabulated_isotherms_degc: vec![300.0],
        ..LayerSelection::default()
    };
    let ts_built = LayerId::WagnerSinglePhasePoints.build(
        DiagramKind::TemperatureEntropy,
        20,
        &isotherm_selection,
    );
    assert!(
        !ts_built.iter().any(|l| l.kind == LayerKind::ComputedCurve),
        "T-s must not pair in an isotherm line -- it is a horizontal line there"
    );
    assert!(
        ts_built
            .iter()
            .any(|l| l.kind == LayerKind::ReferencePoints),
        "T-s must still draw the tabulated points even without the curve"
    );
}

/// The isobar and isotherm layers pair their computed line with the
/// published Wagner rows at the same value, when Wagner tabulated any near it
/// — issue #26: "plot the tabulated data as crosses and iapws97 data as
/// lines... so that a csv plot and graphical plot can be obtained".
///
/// # Methodology
///
/// Builds [`LayerId::Isobars`] and [`LayerId::Isotherms`] and checks, for each
/// of this crate's own `DEFAULT_ISOBARS_BAR` / `DEFAULT_ISOTHERMS_DEGC`
/// values known to coincide with a published Wagner isobar/isotherm (100 bar;
/// 300 °C — both chosen because [`tampines_steam_tables::tabulated_data`]'s
/// own tests already establish they match real rows), that the built layers
/// contain **both** a `ComputedCurve` line and a `ReferencePoints` cross
/// layer whose label names the same value. Also checks that at least one
/// isobar/isotherm value produces *no* companion crosses (5 bar — not a
/// tabulated Wagner isobar), proving empty-on-no-match is real behaviour, not
/// untested.
///
/// # Result (measured 2026-08-21)
///
/// Holds: 100 bar and 300 °C each produce a matching `(line, crosses)` pair;
/// 5 bar produces only its line.
#[cfg(test)]
#[test]
fn isobar_and_isotherm_layers_pair_their_line_with_published_wagner_crosses() {
    let isobars =
        LayerId::Isobars.build(DiagramKind::EnthalpyEntropy, 60, &LayerSelection::default());
    let has_line_and_crosses = |needle: &str| {
        let has_line = isobars
            .iter()
            .any(|l| l.label.contains(needle) && l.kind == LayerKind::ComputedCurve);
        let has_crosses = isobars.iter().any(|l| {
            l.label.contains(needle)
                && l.label.contains("tabulated")
                && l.kind == LayerKind::ReferencePoints
        });
        (has_line, has_crosses)
    };
    let (line_100, crosses_100) = has_line_and_crosses("100 bar");
    assert!(line_100, "expected a computed isobar line at 100 bar");
    assert!(
        crosses_100,
        "expected published Wagner crosses at 100 bar (a tabulated isobar)"
    );
    let (line_5, crosses_5) = has_line_and_crosses("5 bar");
    assert!(line_5, "5 bar's computed line must still be present");
    assert!(
        !crosses_5,
        "5 bar is not a tabulated Wagner isobar -- there must be no crosses layer for it, \
         proving an empty match is real, not fabricated"
    );

    let isotherms =
        LayerId::Isotherms.build(DiagramKind::EnthalpyEntropy, 60, &LayerSelection::default());
    let has_line_300 = isotherms
        .iter()
        .any(|l| l.label.contains("300") && l.kind == LayerKind::ComputedCurve);
    let has_crosses_300 = isotherms.iter().any(|l| {
        l.label.contains("300")
            && l.label.contains("tabulated")
            && l.kind == LayerKind::ReferencePoints
    });
    assert!(
        has_line_300,
        "expected a computed isotherm line at 300 degC"
    );
    assert!(
        has_crosses_300,
        "expected published Wagner crosses at 300 degC (matches rows across many tabulated \
         isobars per tabulated_data's own tests)"
    );
}

/// Checks [`LayerSelection`] actually restricts which isobars/isotherms/
/// quality lines [`LayerId::Isobars`] / [`LayerId::Isotherms`] /
/// [`LayerId::QualityLines`] draw (issue #26's follow-up: *"the isotherm
/// checkbox switches all of the isotherms on... give me a drop-down menu to
/// select which isotherms I want to add and plot"*, later extended to
/// *"I want the same thing done for the quality lines as well"*).
///
/// # Methodology
///
/// Builds all three layers three ways: [`LayerSelection::default`] (every
/// default value, the tool's original all-or-nothing behaviour), a selection
/// naming only one value from each family, and a selection with an empty list
/// for each family. Checks the full selection produces every default value's
/// line, the one-value selection produces exactly that value's line and none
/// of the others, and the empty selection produces no layers at all for that
/// family (not even an empty-labelled one).
///
/// # Result (measured 2026-08-21)
///
/// Holds for isobars, isotherms and quality lines alike.
#[cfg(test)]
#[test]
fn layer_selection_restricts_which_isobars_and_isotherms_are_drawn() {
    let diagram = DiagramKind::EnthalpyEntropy;

    let full = LayerId::Isobars.build(diagram, 20, &LayerSelection::default());
    for p_bar in curves::DEFAULT_ISOBARS_BAR {
        assert!(
            full.iter()
                .any(|l| l.label.contains(&format!("{p_bar} bar"))
                    && l.kind == LayerKind::ComputedCurve),
            "default selection must still draw every default isobar, missing {p_bar} bar"
        );
    }
    let full_quality = LayerId::QualityLines.build(diagram, 20, &LayerSelection::default());
    for x in curves::QUALITY_LINE_VALUES {
        assert!(
            full_quality
                .iter()
                .any(|l| l.label.contains(&format!("x = {x}"))),
            "default selection must still draw every default quality line, missing x = {x}"
        );
    }

    let one = LayerSelection {
        isobars_bar: vec![100.0],
        isotherms_degc: vec![300.0],
        qualities: vec![0.5],
        ..LayerSelection::default()
    };
    let one_isobar = LayerId::Isobars.build(diagram, 20, &one);
    assert!(
        one_isobar
            .iter()
            .any(|l| l.label.contains("100 bar") && l.kind == LayerKind::ComputedCurve),
        "selecting only 100 bar must still draw its line"
    );
    for p_bar in curves::DEFAULT_ISOBARS_BAR {
        if p_bar == 100.0 {
            continue;
        }
        assert!(
            !one_isobar
                .iter()
                .any(|l| l.label.contains(&format!("{p_bar} bar"))),
            "selecting only 100 bar must not draw {p_bar} bar"
        );
    }
    let one_isotherm = LayerId::Isotherms.build(diagram, 20, &one);
    assert!(
        one_isotherm
            .iter()
            .any(|l| l.label.contains("300") && l.kind == LayerKind::ComputedCurve),
        "selecting only 300 degC must still draw its line"
    );
    for t_degc in curves::DEFAULT_ISOTHERMS_DEGC {
        if t_degc == 300.0 {
            continue;
        }
        assert!(
            !one_isotherm
                .iter()
                .any(|l| l.label.contains(&format!("{t_degc} \u{00B0}C"))),
            "selecting only 300 degC must not draw {t_degc} degC"
        );
    }

    let one_quality = LayerId::QualityLines.build(diagram, 20, &one);
    assert!(
        one_quality
            .iter()
            .any(|l| l.label.contains("x = 0.5") && l.kind == LayerKind::ComputedCurve),
        "selecting only x = 0.5 must still draw its line"
    );
    for x in curves::QUALITY_LINE_VALUES {
        if x == 0.5 {
            continue;
        }
        assert!(
            !one_quality
                .iter()
                .any(|l| l.label.contains(&format!("x = {x}"))),
            "selecting only x = 0.5 must not draw x = {x}"
        );
    }

    let empty = LayerSelection {
        isobars_bar: Vec::new(),
        isotherms_degc: Vec::new(),
        qualities: Vec::new(),
        ..LayerSelection::default()
    };
    assert!(
        LayerId::Isobars.build(diagram, 20, &empty).is_empty(),
        "an empty isobar selection must draw nothing"
    );
    assert!(
        LayerId::Isotherms.build(diagram, 20, &empty).is_empty(),
        "an empty isotherm selection must draw nothing"
    );
    assert!(
        LayerId::QualityLines.build(diagram, 20, &empty).is_empty(),
        "an empty quality selection must draw nothing"
    );
}

/// Checks the Compact-legend grouping ([`LayerId::legend_group`]) collapses
/// families the way issue #26's own example does ("Group auxiliary curves
/// where possible: Isotherms / Quality lines / Saturation dome / Validation
/// points") and that every built [`PlotLayer`] actually carries its `LayerId`'s
/// group, not some other string.
///
/// # Methodology
///
/// Asserts the three saturation-envelope layers share one group and every
/// reference-data `LayerId` shares one "Validation / reference data" group
/// (both issue-specified collapses); then builds one layer per family and
/// checks `PlotLayer::legend_group` matches `LayerId::legend_group` exactly,
/// so a future refactor of `base_coloured` cannot silently drop the field.
///
/// # Result (measured 2026-08-20)
///
/// Passes.
#[cfg(test)]
#[test]
fn legend_groups_collapse_the_families_issue_26_names() {
    assert_eq!(
        LayerId::SaturationDome.legend_group(),
        LayerId::SaturatedLiquidLine.legend_group()
    );
    assert_eq!(
        LayerId::SaturationDome.legend_group(),
        LayerId::SaturatedVapourLine.legend_group()
    );

    let reference_group = LayerId::WagnerSaturationPoints.legend_group();
    for layer in LayerId::ALL {
        if layer.kind() == LayerKind::ReferencePoints {
            assert_eq!(
                layer.legend_group(),
                reference_group,
                "{layer:?} should share the one reference-data legend group"
            );
        }
    }

    for layer in LayerId::ALL {
        let built = layer.build(DiagramKind::EnthalpyEntropy, 10, &LayerSelection::default());
        for plot_layer in built {
            // `WagnerSinglePhasePoints` is a deliberate exception since
            // 2026-08-21: it pairs in a computed isobar/isotherm curve
            // alongside its own tabulated points ("please plot the iapws
            // smooth curve along with it"), and that curve correctly reports
            // `LayerId::Isobars`/`Isotherms`'s own legend group -- it is a
            // real isobar/isotherm curve and belongs in that legend row, not
            // a fabricated third group. Only its `ReferencePoints` sub-layer
            // is held to the "must match its own LayerId" rule here.
            if layer == LayerId::WagnerSinglePhasePoints
                && plot_layer.kind == LayerKind::ComputedCurve
            {
                assert!(
                    plot_layer.legend_group == LayerId::Isobars.legend_group()
                        || plot_layer.legend_group == LayerId::Isotherms.legend_group(),
                    "WagnerSinglePhasePoints's paired computed curve must report Isobars' or \
                     Isotherms' own legend group, got {}",
                    plot_layer.legend_group
                );
                continue;
            }
            assert_eq!(
                plot_layer.legend_group,
                layer.legend_group(),
                "{layer:?}'s built PlotLayer.legend_group must match LayerId::legend_group"
            );
        }
    }
}
