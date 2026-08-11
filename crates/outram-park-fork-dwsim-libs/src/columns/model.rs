//! Rigorous-column data model: stages, specifications, condenser/column types,
//! initial estimates, and the solver input/output records.
//!
//! Pure-Rust port of the data-model classes in DWSIM's
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumn.vb` (GPL-3.0), upstream
//! commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright:
//! 2008-2022 Daniel Wagner O. de Medeiros et al.
//!
//! | This module | Upstream class / enum | `RigorousColumn.vb` lines |
//! |---|---|---|
//! | [`Stage`] | `Auxiliary.SepOps.Stage` | 114-313 |
//! | [`InitialEstimates`] | `Auxiliary.SepOps.InitialEstimates` | 315-510 |
//! | [`ColumnSolverInput`] | `ColumnSolverInputData` | 512-556 |
//! | [`ColumnSolverOutput`] | `ColumnSolverOutputData` | 558-574 |
//! | [`StreamKind`] / [`StreamBehavior`] / [`StreamPhase`] | `StreamInformation.Type` / `.Behavior` / `.Phase` | 580-601 |
//! | [`ColumnSpec`] / [`SpecType`] | `Auxiliary.SepOps.ColumnSpec` | 711-816 |
//! | [`ColumnType`] | `Column.ColType` | 1886-1891 |
//! | [`SolvingScheme`] | `Column.SolvingScheme` | 1893-1898 |
//! | [`CondenserType`] | `Column.condtype` | 2410-2414 |
//!
//! # Stage numbering (upstream convention, preserved)
//!
//! Stages are numbered **top to bottom**. Stage `0` is the condenser when the
//! column has one; stage `n_stages - 1` is the reboiler when the column has one
//! (`RigorousColumn.vb` lines 1919-1921, upstream's own comment). Every
//! per-stage vector in [`ColumnSolverInput`] and [`ColumnSolverOutput`] is
//! indexed by that number and has length `n_stages`.
//!
//! Upstream's `ColumnSolverInputData.NumberOfStages` holds the **top index**
//! `ns` (loops read `For i = 0 To ns`). This port instead stores the **count**
//! in [`ColumnSolverInput::number_of_stages`] and derives `ns = count - 1`
//! internally, because an off-by-one in a public field is exactly the kind of
//! thing the workspace "human interface layer" rule exists to prevent.
//!
//! # Units
//!
//! The solver interior is documented raw `f64` in SI, matching the crate's
//! thermo kernel convention (crate `CLAUDE.md`): pressure \[Pa\], temperature
//! \[K\], molar flow \[mol/s\], molar enthalpy \[J/mol\], heat duty \[W\], mole
//! fractions and K-values \[-\], stage efficiency \[-\]. The `uom`-typed public
//! surface is the [`Stage`] accessors and the type aliases at the top of this
//! module.
//!
//! **One deviation from upstream, deliberate:** DWSIM carries feed enthalpies
//! in J/kmol and divides by 1000 inside every solver (`Hfj(i) = HF(i) / 1000`,
//! e.g. `BubblePoint.vb:932`). This port takes feed enthalpies in **J/mol**
//! directly and drops that scaling, so the whole energy balance
//! (`V [mol/s] * H [J/mol] = W`) is dimensionally consistent end to end.
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported from `RigorousColumn.vb`:
//!
//! - `Parameter` (lines 46-112) — a boxed `Double` + XML serialization wrapper
//!   whose only purpose is DWSIM's property-grid/persistence layer. Replaced by
//!   plain `f64` fields.
//! - All `LoadData` / `SaveData` XML serialization and `CloneXML` / `CloneJSON`
//!   (lines 99-112, 271-313, 388-500, 614-694, 735-748, 963-975, 1483-1495,
//!   2074-2230).
//! - Property-grid reflection accessors `GetPropertyValue` / `GetPropertyUnit` /
//!   `SetPropertyValue` (lines 1061-1331, 1566-1767).
//! - Icons/bitmaps and the editor form: `GetIconBitmap`, `DisplayEditForm`,
//!   `UpdateEditForm`, `CloseEditForm`, `GetChartModel*` (lines 1332-1341,
//!   1768-1777, 5589-5685).
//! - `GetReport` / `GeneratePropertiesProfileReport` (lines 1356-1406,
//!   1792-1854, 5261-5419) — text report formatting.
//! - The flowsheet-graph coupling: `ConnectFeed` / `ConnectDistillate` /
//!   `ConnectBottoms` / `CheckConnPos` / `Calculate` / `DeCalculate`
//!   (lines 844-949, 2011-2067, 2638-2747, 4733-5260, 5420-5521) and the two
//!   `GetSolverInputData` builders (lines 2754-4726). Those read and write the
//!   `DWSIM.FlowsheetBase` object graph; this workstream is deliberately
//!   independent of it, so feeds arrive here as plain per-stage data
//!   ([`ColumnSolverInput::feed_flows`] / `feed_compositions` /
//!   `feed_enthalpies`) that a caller assembles however it likes.
//! - `SystemsOfUnits.Converter.ConvertToSI` on spec values (e.g.
//!   `BubblePoint.vb:88`) — spec values are taken in SI here, with the
//!   molar/mass **basis** distinction preserved as [`SpecBasis`].

use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    CatalyticActivity, MolarEnergy, Power, Pressure, Ratio, ThermodynamicTemperature,
};
use uom::si::molar_energy::joule_per_mole;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::Component;

/// Stage pressure \[Pa\]. Alias for readability at the public boundary.
pub type StagePressure = Pressure;

/// Stage temperature \[K\]. Alias for readability at the public boundary.
pub type StageTemperature = ThermodynamicTemperature;

/// Molar flow rate \[mol/s\].
///
/// `uom` 0.38 has no dedicated molar-flow-rate quantity; the katal
/// (`CatalyticActivity`) is dimensionally **exactly** mol/s, so it is aliased
/// here under a name a chemical engineer will recognise. Construct with
/// `MolarFlowRate::new::<uom::si::catalytic_activity::katal>(v)` where `v` is in
/// mol/s.
pub type MolarFlowRate = CatalyticActivity;

/// Molar enthalpy \[J/mol\].
pub type MolarEnthalpy = MolarEnergy;

/// Stage heat duty \[W\] — positive **into** the stage (upstream sign
/// convention: the condenser duty `Q(0)` comes out negative for a normal
/// distillation column, the reboiler duty `Q(ns)` positive).
pub type StageHeatDuty = Power;

/// Murphree-style stage efficiency \[-\], `0 < eta <= 1`.
pub type StageEfficiency = Ratio;

/// Errors raised by the rigorous-column model and its solvers.
///
/// Upstream throws bare `Exception`s carrying localised UI strings
/// (`GetTranslatedString("DCMaxIterationsReached")` etc.); this port returns a
/// typed error so a caller can distinguish "did not converge" from "the input
/// was malformed" without string matching.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ColumnError {
    /// Two collections that must share a length do not.
    #[error("{what}: expected length {expected}, found {found}")]
    LengthMismatch {
        /// Which collection pair disagreed.
        what: &'static str,
        /// The length that was required.
        expected: usize,
        /// The length that was supplied.
        found: usize,
    },
    /// A column needs at least 2 stages (upstream's editor enforces `n > 3`;
    /// the solvers themselves only need a top and a bottom stage).
    #[error("a column needs at least 2 stages, got {found}")]
    TooFewStages {
        /// Number of stages supplied.
        found: usize,
    },
    /// The tridiagonal elimination hit a zero/non-finite pivot.
    #[error("tridiagonal system is singular at row {row}")]
    SingularMatrix {
        /// Row index at which the pivot vanished.
        row: usize,
    },
    /// The inner (or outer) iteration ran out of budget.
    ///
    /// Ports upstream's `DCMaxIterationsReached`
    /// (`BubblePoint.vb:1701`, `SumRates.vb:696`, `NewtonRaphson.vb:1154`).
    #[error("column solver did not converge in {iterations} iterations (final error {error:e})")]
    NotConverged {
        /// Iterations actually taken.
        iterations: usize,
        /// Final value of the solver's error function.
        error: f64,
    },
    /// A stage profile went non-finite or unphysical (negative flow, negative
    /// absolute temperature, composition that will not normalise).
    ///
    /// Ports upstream's `DCGeneralError` and the "Could not converge to a valid
    /// solution" mass-balance guard (`BubblePoint.vb:1726`, `:1818`).
    #[error("invalid column profile at stage {stage}: {detail}")]
    InvalidProfile {
        /// Stage index where the check failed.
        stage: usize,
        /// What was wrong.
        detail: String,
    },
    /// A stage bubble-point temperature calculation failed.
    ///
    /// Ports upstream's "Error calculating bubble point temperature for stage
    /// {0} with P = {1} Pa" (`BubblePoint.vb:1283`).
    #[error("bubble-point temperature failed on stage {stage} at P = {pressure} Pa: {detail}")]
    BubblePointFailed {
        /// Stage index.
        stage: usize,
        /// Stage pressure \[Pa\].
        pressure: f64,
        /// Underlying cause.
        detail: String,
    },
    /// The equilibrium converged to the trivial solution `K_i = 1` for all `i`.
    ///
    /// Ports `SumRates.vb:792` (`AUX_CheckTrivial`).
    #[error("converged to the trivial solution (all K-values ~ 1) on stage {stage}")]
    TrivialSolution {
        /// Stage index.
        stage: usize,
    },
    /// A specification was not usable — e.g. a component-indexed spec whose
    /// index is out of range, or a spec value that must be positive but is not.
    #[error("invalid column specification: {detail}")]
    InvalidSpec {
        /// What was wrong with the spec.
        detail: String,
    },
}

/// Condenser configuration — DWSIM's `Column.condtype`
/// (`RigorousColumn.vb` lines 2410-2414).
///
/// Selects what leaves the top stage and therefore which mass balance closes
/// there. Enum dispatch, no `dyn`, per the workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CondenserType {
    /// Total condenser: all overhead vapour is condensed. The distillate is a
    /// **liquid** side draw `LSS_0`, and the top-stage vapour flow `V_0` is
    /// zero. This is upstream's default (`CondenserType = Total_Condenser`,
    /// `RigorousColumn.vb:1929`).
    #[default]
    TotalCondenser,
    /// Partial condenser: part of the overhead leaves as vapour `V_0`, the rest
    /// is condensed and split between reflux `L_0` and a liquid distillate
    /// `LSS_0`.
    PartialCondenser,
    /// Full reflux: all condensed liquid is returned as reflux and the only
    /// overhead product is the vapour `V_0`; `LSS_0 = 0`.
    FullReflux,
}

/// Column configuration — DWSIM's `Column.ColType`
/// (`RigorousColumn.vb` lines 1886-1891).
///
/// Determines which end-stage heat duties the solver computes from the energy
/// balance versus takes as given (`BubblePoint.vb:1645-1673`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnType {
    /// Condenser at the top **and** reboiler at the bottom; both duties are
    /// back-calculated unless specified.
    #[default]
    DistillationColumn,
    /// Neither condenser nor reboiler; both end duties are taken as user input.
    AbsorptionColumn,
    /// Reboiler only (no condenser); only `Q(ns)` is back-calculated.
    ReboiledAbsorber,
    /// Condenser only (no reboiler); only `Q(0)` is back-calculated.
    RefluxedAbsorber,
}

/// Initialisation strategy — DWSIM's `Column.SolvingScheme`
/// (`RigorousColumn.vb` lines 1893-1898).
///
/// Upstream runs the chosen rigorous solver against an *ideal* property model
/// first (`IdealK` / `IdealH` flags threaded through every solver) so the
/// profile is warm before the real thermodynamics is switched on. The flags
/// also suppress the max-iteration abort during those warm-up passes
/// (`BubblePoint.vb:1679`, `SumRates.vb:694`).
///
/// This port carries the enum and honours the max-iteration-abort suppression;
/// what it does **not** yet do is substitute a Raoult package for the
/// warm-up passes — see [`SolvingScheme`]'s variant docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolvingScheme {
    /// Warm up with ideal K-values, then solve with the real package.
    /// **Not yet honoured** in this port (treated as [`Self::Direct`]).
    IdealKInit,
    /// Warm up with ideal enthalpies. **Not yet honoured** (treated as
    /// [`Self::Direct`]).
    IdealEnthalpyInit,
    /// Warm up with both ideal K-values and ideal enthalpies. **Not yet
    /// honoured** (treated as [`Self::Direct`]).
    IdealKAndEnthalpyInit,
    /// Go straight at the real property package. Upstream's default
    /// (`SolverScheme = SolvingScheme.Direct`, `RigorousColumn.vb:1917`) and
    /// the only variant this port actually implements.
    #[default]
    Direct,
}

/// What kind of stream a column connection carries — DWSIM's
/// `StreamInformation.Type` (`RigorousColumn.vb` lines 580-583).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// A material stream (mass + energy).
    Material,
    /// An energy stream (duty only).
    Energy,
}

/// The role a connected stream plays — DWSIM's `StreamInformation.Behavior`
/// (`RigorousColumn.vb` lines 585-595).
///
/// Carried for model fidelity and for callers assembling a column from a
/// flowsheet; the solvers themselves consume only the resulting per-stage feed
/// and side-draw vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamBehavior {
    /// Liquid product from the condenser.
    Distillate,
    /// Liquid product from the reboiler.
    BottomsLiquid,
    /// A feed entering some stage.
    Feed,
    /// A side draw from some stage.
    Sidedraw,
    /// Vapour product from the top stage.
    OverheadVapor,
    /// Liquid product of a side operation (side stripper / rectifier).
    SideOpLiquidProduct,
    /// Vapour product of a side operation.
    SideOpVaporProduct,
    /// Live-steam injection.
    Steam,
    /// An inter-stage heat exchanger (pumparound duty).
    InterExchanger,
}

/// Which phase a connected stream is drawn from — DWSIM's
/// `StreamInformation.Phase` (`RigorousColumn.vb` lines 597-601).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPhase {
    /// Liquid.
    Liquid,
    /// Vapour.
    Vapor,
    /// Both (mixed).
    Both,
}

/// The quantity a column specification fixes — DWSIM's `ColumnSpec.SpecType`
/// (`RigorousColumn.vb` lines 715-726).
///
/// A rigorous distillation column has exactly **two** degrees of freedom once
/// the feeds, pressures and stage count are fixed: one condenser-end spec and
/// one reboiler-end spec. Upstream keys them `"C"` and `"R"` in a dictionary;
/// this port names them [`ColumnSolverInput::condenser_spec`] and
/// [`ColumnSolverInput::reboiler_spec`].
///
/// The solvers split these into two classes
/// (`BubblePoint.vb:103-124`): specs the bubble-point inner loop can impose
/// **directly** on the mass balance ([`Self::HeatDuty`],
/// [`Self::ProductMolarFlowRate`], [`Self::ProductMassFlowRate`],
/// [`Self::StreamRatio`], [`Self::FeedRecovery`]), and specs that need an
/// **outer root-find** on top of it (the component-indexed ones and
/// [`Self::Temperature`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecType {
    /// Fix the end-stage heat duty \[W\]. Directly imposable.
    #[default]
    HeatDuty,
    /// Fix the end product's total molar flow \[mol/s\]. Directly imposable.
    ProductMolarFlowRate,
    /// Fix one component's molar flow in the end product \[mol/s\]. Needs the
    /// outer loop.
    ComponentMolarFlowRate,
    /// Fix the end product's total mass flow \[kg/s\]. Directly imposable
    /// (converted to molar with the end-stage mixture molar mass).
    ProductMassFlowRate,
    /// Fix one component's mass flow in the end product \[kg/s\]. Needs the
    /// outer loop.
    ComponentMassFlowRate,
    /// Fix one component's fraction (molar or mass, see [`SpecBasis`]) in the
    /// end product \[-\]. Needs the outer loop.
    ComponentFraction,
    /// Fix one component's recovery, in **percent** of that component's total
    /// feed rate, into the end product. Needs the outer loop.
    ComponentRecovery,
    /// Fix a stream ratio \[-\]: at the condenser end this is the **reflux
    /// ratio** `L_0 / D`; at the reboiler end it is the **boil-up ratio**
    /// `V_ns / L_ns`. Directly imposable at the condenser end only.
    StreamRatio,
    /// Fix the end-stage temperature \[K\]. Needs the outer loop.
    Temperature,
    /// Fix the end product's flow as a **percentage** of the total feed molar
    /// flow. Directly imposable.
    FeedRecovery,
}

/// Whether a fraction/flow spec is on a molar or mass basis — DWSIM's
/// `ColumnSpec.SpecUnit` string, which the solvers test for `"M"`/`"Molar"`
/// versus `"W"` (`BubblePoint.vb:218`, `:270`).
///
/// Modelled as an enum rather than a magic string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecBasis {
    /// Molar basis (upstream `"M"` / `"Molar"`).
    #[default]
    Molar,
    /// Mass basis (upstream `"W"` / `"Mass"`).
    Mass,
}

/// One end-of-column specification — DWSIM's `Auxiliary.SepOps.ColumnSpec`
/// (`RigorousColumn.vb` lines 711-816).
///
/// # Units
///
/// [`Self::value`] is in SI for the [`SpecType`] chosen: W for
/// [`SpecType::HeatDuty`], mol/s for the molar-flow specs, kg/s for the
/// mass-flow specs, K for [`SpecType::Temperature`], and dimensionless for
/// [`SpecType::ComponentFraction`] and [`SpecType::StreamRatio`].
/// [`SpecType::ComponentRecovery`] and [`SpecType::FeedRecovery`] are in
/// **percent** (upstream divides by 100 internally, `BubblePoint.vb:251`,
/// `:888`).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSpec {
    /// Which quantity is fixed.
    pub spec_type: SpecType,
    /// The target value, units per [`SpecType`] (see the struct docs).
    pub value: f64,
    /// Molar or mass basis, for the fraction and product-flow specs.
    pub basis: SpecBasis,
    /// Which component the spec refers to, for the component-indexed spec
    /// types. Ignored otherwise. Must be `< n_components`.
    pub component_index: usize,
    /// Stage the spec applies to. Upstream carries it but the solvers only ever
    /// use stage `0` (condenser) or `ns` (reboiler); kept for fidelity.
    pub stage_number: usize,
    /// An optional user starting guess for the outer root-find, used when a
    /// spec needs one. Ports `ColumnSpec.InitialEstimate`
    /// (`RigorousColumn.vb:816`, consumed at `BubblePoint.vb:137`).
    pub initial_estimate: Option<f64>,
    /// The value the solver actually achieved, written back after a solve.
    /// Ports `ColumnSpec.CalculatedValue` (`RigorousColumn.vb:814`).
    pub calculated_value: f64,
}

impl Default for ColumnSpec {
    fn default() -> Self {
        Self {
            spec_type: SpecType::HeatDuty,
            value: 0.0,
            basis: SpecBasis::Molar,
            component_index: 0,
            stage_number: 0,
            initial_estimate: None,
            calculated_value: 0.0,
        }
    }
}

impl ColumnSpec {
    /// A reflux-ratio (condenser-end [`SpecType::StreamRatio`]) spec.
    ///
    /// `reflux_ratio` = `L_0 / D` \[-\], must be > 0. This is the cheapest spec
    /// for the bubble-point solvers: it is imposed directly inside the inner
    /// loop with no outer root-find (`BubblePoint.vb:985-986`).
    #[must_use]
    pub fn reflux_ratio(reflux_ratio: f64) -> Self {
        Self {
            spec_type: SpecType::StreamRatio,
            value: reflux_ratio,
            stage_number: 0,
            ..Self::default()
        }
    }

    /// A bottoms (or distillate) molar-flow spec \[mol/s\].
    ///
    /// Directly imposable (`BubblePoint.vb:1003-1004`).
    #[must_use]
    pub fn product_molar_flow(molar_flow: MolarFlowRate) -> Self {
        Self {
            spec_type: SpecType::ProductMolarFlowRate,
            value: molar_flow.get::<katal>(),
            ..Self::default()
        }
    }

    /// An end-stage heat-duty spec \[W\].
    ///
    /// Directly imposable (`BubblePoint.vb:987-990`).
    #[must_use]
    pub fn heat_duty(duty: StageHeatDuty) -> Self {
        Self {
            spec_type: SpecType::HeatDuty,
            value: duty.get::<watt>(),
            ..Self::default()
        }
    }

    /// An end-product component mole-fraction spec \[-\].
    ///
    /// Requires the outer root-find (`BubblePoint.vb:104-112`).
    #[must_use]
    pub fn component_mole_fraction(component_index: usize, fraction: f64) -> Self {
        Self {
            spec_type: SpecType::ComponentFraction,
            value: fraction,
            basis: SpecBasis::Molar,
            component_index,
            ..Self::default()
        }
    }

    /// `true` if the bubble-point inner loop can impose this spec directly,
    /// `false` if it needs the outer root-find.
    ///
    /// Ports the condenser-end classification of `BubblePoint.vb:103-112`.
    #[must_use]
    pub fn directly_imposable_at_condenser(&self) -> bool {
        !matches!(
            self.spec_type,
            SpecType::ComponentFraction
                | SpecType::ComponentMassFlowRate
                | SpecType::ComponentMolarFlowRate
                | SpecType::ComponentRecovery
                | SpecType::Temperature
        )
    }

    /// As [`Self::directly_imposable_at_condenser`], but for the reboiler end,
    /// where [`SpecType::StreamRatio`] (the boil-up ratio) *also* needs the
    /// outer loop — ports `BubblePoint.vb:114-124`.
    #[must_use]
    pub fn directly_imposable_at_reboiler(&self) -> bool {
        self.directly_imposable_at_condenser() && self.spec_type != SpecType::StreamRatio
    }
}

/// One equilibrium stage — DWSIM's `Auxiliary.SepOps.Stage`
/// (`RigorousColumn.vb` lines 114-313).
///
/// Holds the *given* per-stage conditions (pressure, heat duty, efficiency,
/// side draws, feed) plus the current temperature estimate. The per-component
/// K-value / liquid / vapour dictionaries upstream keeps on the stage
/// (`Stage.Kvalues`, `.l`, `.v`) are **not** mirrored here: this port keeps
/// those in the flat `Vec<Vec<f64>>` profiles of [`ColumnSolverInput`] /
/// [`ColumnSolverOutput`], which is what the solvers actually read, and avoids
/// carrying the same numbers in two places.
///
/// # Units / valid ranges
///
/// - `pressure` \[Pa\] > 0.
/// - `temperature` \[K\] > 0 — an *estimate* on input, a result on output.
/// - `efficiency` \[-\] in `(0, 1]`; `1.0` is an ideal equilibrium stage.
///   Upstream's default is `1.0` (`RigorousColumn.vb:119`).
/// - `heat_duty` \[W\], positive **into** the stage.
/// - `feed_molar_flow`, `vapor_side_draw`, `liquid_side_draw` \[mol/s\] >= 0.
/// - `feed_composition` — mole fractions \[-\], length `n_components`, summing
///   to 1 for a physical feed (all zeros for a stage with no feed).
/// - `feed_molar_enthalpy` \[J/mol\] of the feed at its own condition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stage {
    /// Human-readable stage name (upstream `Stage.Name`).
    pub name: String,
    /// Stage pressure \[Pa\].
    pub pressure: f64,
    /// Stage temperature \[K\] — estimate in, result out.
    pub temperature: f64,
    /// Stage efficiency \[-\], `(0, 1]`.
    pub efficiency: f64,
    /// Stage heat duty \[W\], positive into the stage.
    pub heat_duty: f64,
    /// Feed molar flow onto this stage \[mol/s\].
    pub feed_molar_flow: f64,
    /// Feed mole fractions \[-\], length `n_components`.
    pub feed_composition: Vec<f64>,
    /// Feed molar enthalpy \[J/mol\].
    pub feed_molar_enthalpy: f64,
    /// Vapour side draw from this stage \[mol/s\] (upstream `Stage.Vss`).
    pub vapor_side_draw: f64,
    /// Liquid side draw from this stage \[mol/s\] (upstream `Stage.Lss`).
    pub liquid_side_draw: f64,
}

impl Stage {
    /// A bare stage with no feed, no side draws, no duty, unit efficiency.
    ///
    /// `pressure` and `temperature` are `uom`-typed at this boundary and stored
    /// internally as Pa / K.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        pressure: StagePressure,
        temperature: StageTemperature,
        n_components: usize,
    ) -> Self {
        Self {
            name: name.into(),
            pressure: pressure.get::<pascal>(),
            temperature: temperature.get::<kelvin>(),
            efficiency: 1.0,
            heat_duty: 0.0,
            feed_molar_flow: 0.0,
            feed_composition: vec![0.0; n_components],
            feed_molar_enthalpy: 0.0,
            vapor_side_draw: 0.0,
            liquid_side_draw: 0.0,
        }
    }

    /// Attach a feed to this stage.
    ///
    /// `composition` are mole fractions \[-\] and must have length
    /// `n_components`; `enthalpy` is the feed's molar enthalpy \[J/mol\] on the
    /// **same reference state** the column thermo uses (see
    /// [`crate::columns::thermo_bridge::ColumnThermo`]).
    #[must_use]
    pub fn with_feed(
        mut self,
        molar_flow: MolarFlowRate,
        composition: Vec<f64>,
        enthalpy: MolarEnthalpy,
    ) -> Self {
        self.feed_molar_flow = molar_flow.get::<katal>();
        self.feed_composition = composition;
        self.feed_molar_enthalpy = enthalpy.get::<joule_per_mole>();
        self
    }

    /// Set the stage efficiency \[-\], `(0, 1]`.
    #[must_use]
    pub fn with_efficiency(mut self, efficiency: StageEfficiency) -> Self {
        self.efficiency = efficiency.get::<ratio>();
        self
    }

    /// Set the stage heat duty \[W\], positive into the stage.
    #[must_use]
    pub fn with_heat_duty(mut self, duty: StageHeatDuty) -> Self {
        self.heat_duty = duty.get::<watt>();
        self
    }

    /// Set the liquid and vapour side draws \[mol/s\].
    #[must_use]
    pub fn with_side_draws(mut self, liquid: MolarFlowRate, vapor: MolarFlowRate) -> Self {
        self.liquid_side_draw = liquid.get::<katal>();
        self.vapor_side_draw = vapor.get::<katal>();
        self
    }

    /// Stage pressure as a `uom` quantity.
    #[must_use]
    pub fn pressure(&self) -> StagePressure {
        StagePressure::new::<pascal>(self.pressure)
    }

    /// Stage temperature as a `uom` quantity.
    #[must_use]
    pub fn temperature(&self) -> StageTemperature {
        StageTemperature::new::<kelvin>(self.temperature)
    }
}

/// User-supplied (or previously-converged) starting profiles — DWSIM's
/// `Auxiliary.SepOps.InitialEstimates` (`RigorousColumn.vb` lines 315-510).
///
/// Every field is optional; whatever is absent is generated by
/// [`crate::columns::initial_estimates::generate`]. Upstream's four
/// `Validate*` methods (lines 330-387) are ported as [`Self::validate`].
///
/// # Units
///
/// `stage_temperatures` \[K\], `liquid_molar_flows` / `vapor_molar_flows`
/// \[mol/s\], compositions \[-\] (outer index = stage, inner = component),
/// `reflux_ratio` \[-\], the three product flow rates \[mol/s\].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InitialEstimates {
    /// Per-stage temperature estimates \[K\].
    pub stage_temperatures: Vec<f64>,
    /// Per-stage liquid molar flow estimates \[mol/s\].
    pub liquid_molar_flows: Vec<f64>,
    /// Per-stage vapour molar flow estimates \[mol/s\].
    pub vapor_molar_flows: Vec<f64>,
    /// Per-stage liquid mole-fraction estimates \[-\].
    pub liquid_compositions: Vec<Vec<f64>>,
    /// Per-stage vapour mole-fraction estimates \[-\].
    pub vapor_compositions: Vec<Vec<f64>>,
    /// Overhead vapour product flow estimate \[mol/s\].
    pub vapor_product_flow_rate: Option<f64>,
    /// Distillate flow estimate \[mol/s\].
    pub distillate_flow_rate: Option<f64>,
    /// Bottoms flow estimate \[mol/s\].
    pub bottoms_flow_rate: Option<f64>,
    /// Reflux-ratio estimate \[-\].
    pub reflux_ratio: Option<f64>,
}

impl InitialEstimates {
    /// Are the temperature estimates usable?
    ///
    /// Ports `ValidateTemperatures` (`RigorousColumn.vb:330-340`): non-empty,
    /// non-zero sum, all finite. This port additionally rejects non-positive
    /// absolute temperatures, which upstream's `IsValid` does not check.
    #[must_use]
    pub fn temperatures_valid(&self) -> bool {
        !self.stage_temperatures.is_empty()
            && self.stage_temperatures.iter().sum::<f64>() != 0.0
            && self
                .stage_temperatures
                .iter()
                .all(|t| t.is_finite() && *t > 0.0)
    }

    /// Are the vapour-flow estimates usable?
    /// Ports `ValidateVaporFlows` (`RigorousColumn.vb:342-352`).
    #[must_use]
    pub fn vapor_flows_valid(&self) -> bool {
        !self.vapor_molar_flows.is_empty()
            && self.vapor_molar_flows.iter().sum::<f64>() != 0.0
            && self.vapor_molar_flows.iter().all(|v| v.is_finite())
    }

    /// Are the liquid-flow estimates usable?
    /// Ports `ValidateLiquidFlows` (`RigorousColumn.vb:354-364`).
    #[must_use]
    pub fn liquid_flows_valid(&self) -> bool {
        !self.liquid_molar_flows.is_empty()
            && self.liquid_molar_flows.iter().sum::<f64>() != 0.0
            && self.liquid_molar_flows.iter().all(|l| l.is_finite())
    }

    /// Are both composition profiles usable?
    /// Ports `ValidateCompositions` (`RigorousColumn.vb:366-386`).
    #[must_use]
    pub fn compositions_valid(&self) -> bool {
        let ok = |rows: &Vec<Vec<f64>>| {
            !rows.is_empty()
                && rows.iter().all(|r| {
                    !r.is_empty() && r.iter().all(|v| v.is_finite()) && r.iter().sum::<f64>() > 0.0
                })
        };
        ok(&self.liquid_compositions) && ok(&self.vapor_compositions)
    }

    /// All four validations at once.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.temperatures_valid()
            && self.vapor_flows_valid()
            && self.liquid_flows_valid()
            && self.compositions_valid()
    }
}

/// Everything a column solver needs — DWSIM's `ColumnSolverInputData`
/// (`RigorousColumn.vb` lines 512-556).
///
/// # Layout
///
/// Every `Vec<f64>` is indexed by stage and has length
/// [`Self::number_of_stages`]. Every `Vec<Vec<f64>>` is `[stage][component]`,
/// with the inner length equal to `components.len()`.
///
/// # Units
///
/// `stage_temperatures` \[K\], `stage_pressures` \[Pa\], `stage_heats` \[W\],
/// `stage_efficiencies` \[-\], all flows \[mol/s\], `feed_enthalpies`
/// \[J/mol\], all compositions and `k_values` \[-\].
///
/// # Fields upstream has that this port does not
///
/// `ColumnObject` (the flowsheet back-reference), `CalculationMode`, and the
/// `L1trials` / `L2trials` / `x1trials` / `x2trials` liquid-liquid trial-phase
/// seeds (lines 514, 516, 549-552) — the latter feed DWSIM's liquid-liquid
/// extractor mode, which this port does not implement (see the module header of
/// [`crate::columns`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSolverInput {
    /// Pure-component constants, length `n_components`.
    pub components: Vec<Component>,
    /// Thermodynamic model used for K-values and enthalpy departures.
    pub package: PropertyPackageModel,
    /// **Count** of stages (upstream stores the top index; see the module
    /// header). Must be >= 2.
    pub number_of_stages: usize,
    /// Inner-loop iteration budget (upstream default 100,
    /// `RigorousColumn.vb:1912`).
    pub max_iterations: usize,
    /// Convergence tolerances. Index 0 is the inner-loop tolerance used by the
    /// bubble-point solvers; index 1 is the external-loop tolerance used by
    /// sum-rates and Naphtali-Sandholm (upstream both default to 1e-5,
    /// `RigorousColumn.vb:1913-1914`).
    pub tolerances: Vec<f64>,
    /// Stop after this many inner iterations regardless of convergence, when
    /// `Some`. Ports `EarlyStopIteration` / the `stopatitnumber` argument
    /// (`RigorousColumn.vb:521`, `BubblePoint.vb:1730`), used to run a short
    /// bubble-point warm-up before the Newton solver.
    pub early_stop_iteration: Option<usize>,
    /// Stage temperature estimates \[K\].
    pub stage_temperatures: Vec<f64>,
    /// Stage pressures \[Pa\] (given, never solved for).
    pub stage_pressures: Vec<f64>,
    /// Stage heat duties \[W\] (given for interior stages; the end duties are
    /// back-calculated unless specified).
    pub stage_heats: Vec<f64>,
    /// Stage efficiencies \[-\].
    pub stage_efficiencies: Vec<f64>,
    /// Feed molar flows \[mol/s\].
    pub feed_flows: Vec<f64>,
    /// Feed mole fractions \[-\], `[stage][component]`.
    pub feed_compositions: Vec<Vec<f64>>,
    /// Feed molar enthalpies \[J/mol\].
    pub feed_enthalpies: Vec<f64>,
    /// Vapour molar flow estimates \[mol/s\].
    pub vapor_flows: Vec<f64>,
    /// Vapour mole-fraction estimates \[-\].
    pub vapor_compositions: Vec<Vec<f64>>,
    /// Liquid molar flow estimates \[mol/s\].
    pub liquid_flows: Vec<f64>,
    /// Liquid mole-fraction estimates \[-\].
    pub liquid_compositions: Vec<Vec<f64>>,
    /// Vapour side draws \[mol/s\].
    pub vapor_side_draws: Vec<f64>,
    /// Liquid side draws \[mol/s\]. Index 0 doubles as the distillate rate for
    /// a total/partial condenser.
    pub liquid_side_draws: Vec<f64>,
    /// K-value estimates \[-\], `[stage][component]`.
    pub k_values: Vec<Vec<f64>>,
    /// Overall (feed-basis) mole fractions per stage \[-\], used only by the
    /// component-recovery specs (`BufferPoint.vb:254`).
    pub overall_compositions: Vec<Vec<f64>>,
    /// Condenser configuration.
    pub condenser_type: CondenserType,
    /// Column configuration.
    pub column_type: ColumnType,
    /// The condenser-end (upstream `"C"`) specification.
    pub condenser_spec: ColumnSpec,
    /// The reboiler-end (upstream `"R"`) specification.
    pub reboiler_spec: ColumnSpec,
    /// Condenser sub-cooling \[K\] below the bubble point; 0 for a saturated
    /// condenser. Ports `SubcoolingDeltaT` (`RigorousColumn.vb:554`, applied at
    /// `BubblePoint.vb:1295`).
    pub subcooling_delta_t: f64,
}

impl ColumnSolverInput {
    /// Number of components.
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.components.len()
    }

    /// Top stage index `ns` (upstream's `NumberOfStages`).
    #[must_use]
    pub fn top_index(&self) -> usize {
        self.number_of_stages.saturating_sub(1)
    }

    /// The inner-loop tolerance (`tolerances[0]`, defaulting to 1e-5).
    #[must_use]
    pub fn inner_tolerance(&self) -> f64 {
        self.tolerances.first().copied().unwrap_or(1e-5)
    }

    /// The external-loop tolerance (`tolerances[1]`, falling back to the inner
    /// one). Ports upstream's `tol.MinY_NonZero()` usage in
    /// `NewtonRaphson.vb:1071` and `tol(1)` in `SumRates.vb:788`.
    #[must_use]
    pub fn outer_tolerance(&self) -> f64 {
        self.tolerances
            .get(1)
            .copied()
            .filter(|t| *t > 0.0)
            .unwrap_or_else(|| self.inner_tolerance())
    }

    /// Check every per-stage / per-component collection has the right shape.
    ///
    /// # Errors
    ///
    /// [`ColumnError::TooFewStages`] for fewer than 2 stages, or
    /// [`ColumnError::LengthMismatch`] naming the first collection whose length
    /// is wrong.
    pub fn validate_shape(&self) -> Result<(), ColumnError> {
        let ns = self.number_of_stages;
        if ns < 2 {
            return Err(ColumnError::TooFewStages { found: ns });
        }
        let nc = self.n_components();
        let flat: [(&'static str, usize); 10] = [
            ("stage_temperatures", self.stage_temperatures.len()),
            ("stage_pressures", self.stage_pressures.len()),
            ("stage_heats", self.stage_heats.len()),
            ("stage_efficiencies", self.stage_efficiencies.len()),
            ("feed_flows", self.feed_flows.len()),
            ("feed_enthalpies", self.feed_enthalpies.len()),
            ("vapor_flows", self.vapor_flows.len()),
            ("liquid_flows", self.liquid_flows.len()),
            ("vapor_side_draws", self.vapor_side_draws.len()),
            ("liquid_side_draws", self.liquid_side_draws.len()),
        ];
        for (what, found) in flat {
            if found != ns {
                return Err(ColumnError::LengthMismatch {
                    what,
                    expected: ns,
                    found,
                });
            }
        }
        let nested: [(&'static str, &Vec<Vec<f64>>); 5] = [
            ("feed_compositions", &self.feed_compositions),
            ("vapor_compositions", &self.vapor_compositions),
            ("liquid_compositions", &self.liquid_compositions),
            ("k_values", &self.k_values),
            ("overall_compositions", &self.overall_compositions),
        ];
        for (what, rows) in nested {
            if rows.len() != ns {
                return Err(ColumnError::LengthMismatch {
                    what,
                    expected: ns,
                    found: rows.len(),
                });
            }
            for row in rows {
                if row.len() != nc {
                    return Err(ColumnError::LengthMismatch {
                        what,
                        expected: nc,
                        found: row.len(),
                    });
                }
            }
        }
        for spec in [&self.condenser_spec, &self.reboiler_spec] {
            if spec.component_index >= nc {
                return Err(ColumnError::InvalidSpec {
                    detail: format!(
                        "component_index {} is out of range for {nc} components",
                        spec.component_index
                    ),
                });
            }
        }
        Ok(())
    }
}

/// A converged (or best-effort) column profile — DWSIM's
/// `ColumnSolverOutputData` (`RigorousColumn.vb` lines 558-574).
///
/// Every `Vec` is indexed by stage, length `number_of_stages`; nested vectors
/// are `[stage][component]`. Units match [`ColumnSolverInput`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColumnSolverOutput {
    /// Inner iterations taken.
    pub iterations_taken: usize,
    /// Final value of the solver's error function (its meaning is
    /// solver-specific: sum of squared temperature changes for the bubble-point
    /// and sum-rates solvers, sum of squared MESH residuals for
    /// Naphtali-Sandholm).
    pub final_error: f64,
    /// Converged stage temperatures \[K\].
    pub stage_temperatures: Vec<f64>,
    /// Condenser / interior / reboiler heat duties \[W\].
    pub stage_heats: Vec<f64>,
    /// Stage vapour molar flows \[mol/s\].
    pub vapor_flows: Vec<f64>,
    /// Stage vapour mole fractions \[-\].
    pub vapor_compositions: Vec<Vec<f64>>,
    /// Stage liquid molar flows \[mol/s\].
    pub liquid_flows: Vec<f64>,
    /// Stage liquid mole fractions \[-\].
    pub liquid_compositions: Vec<Vec<f64>>,
    /// Vapour side draws \[mol/s\].
    pub vapor_side_draws: Vec<f64>,
    /// Liquid side draws \[mol/s\]; index 0 is the distillate for a
    /// total/partial condenser.
    pub liquid_side_draws: Vec<f64>,
    /// Stage K-values \[-\].
    pub k_values: Vec<Vec<f64>>,
    /// The condenser spec with its `calculated_value` filled in.
    pub condenser_spec: ColumnSpec,
    /// The reboiler spec with its `calculated_value` filled in.
    pub reboiler_spec: ColumnSpec,
}

impl ColumnSolverOutput {
    /// Distillate molar flow \[mol/s\] as a `uom` quantity — the liquid side
    /// draw off stage 0 for a total/partial condenser, or the top vapour flow
    /// for a full-reflux column.
    #[must_use]
    pub fn distillate_molar_flow(&self, condenser: CondenserType) -> MolarFlowRate {
        let v = match condenser {
            CondenserType::FullReflux => self.vapor_flows.first().copied().unwrap_or(0.0),
            _ => self.liquid_side_draws.first().copied().unwrap_or(0.0),
        };
        MolarFlowRate::new::<katal>(v)
    }

    /// Bottoms molar flow \[mol/s\] as a `uom` quantity — the liquid leaving
    /// the last stage.
    #[must_use]
    pub fn bottoms_molar_flow(&self) -> MolarFlowRate {
        MolarFlowRate::new::<katal>(self.liquid_flows.last().copied().unwrap_or(0.0))
    }

    /// Condenser duty \[W\] as a `uom` quantity (stage 0).
    #[must_use]
    pub fn condenser_duty(&self) -> StageHeatDuty {
        StageHeatDuty::new::<watt>(self.stage_heats.first().copied().unwrap_or(0.0))
    }

    /// Reboiler duty \[W\] as a `uom` quantity (last stage).
    #[must_use]
    pub fn reboiler_duty(&self) -> StageHeatDuty {
        StageHeatDuty::new::<watt>(self.stage_heats.last().copied().unwrap_or(0.0))
    }

    /// Overall molar balance residual \[mol/s\]:
    /// `sum(F) - sum(LSS) - sum(VSS) - V_0 - L_ns`.
    ///
    /// Zero (to round-off) for a converged column. This is the check upstream
    /// performs only implicitly through the composition-sum guard
    /// (`BubblePoint.vb:1791-1820`); exposing it makes the mass-balance closure
    /// testable, which the V&V rule requires.
    #[must_use]
    pub fn molar_balance_residual(&self, feed_flows: &[f64]) -> f64 {
        let sum_f: f64 = feed_flows.iter().sum();
        let sum_lss: f64 = self.liquid_side_draws.iter().skip(1).sum();
        let sum_vss: f64 = self.vapor_side_draws.iter().sum();
        let d = self.liquid_side_draws.first().copied().unwrap_or(0.0);
        let v0 = self.vapor_flows.first().copied().unwrap_or(0.0);
        let b = self.liquid_flows.last().copied().unwrap_or(0.0);
        sum_f - sum_lss - sum_vss - d - v0 - b
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the column data model
    //!
    //! **Methodology.** These are structural checks on the ported data model,
    //! not physics: the spec classification must reproduce upstream's
    //! `specC_OK` / `specR_OK` tables (`BubblePoint.vb:103-124`), the
    //! shape validation must catch every wrong-length collection, and the
    //! `uom` constructors must round-trip to the documented SI values.
    //!
    //! **Results (2026-08-11, release build):** all three tests pass.

    use super::*;

    /// **Methodology.** Reproduce upstream's spec classification tables. At the
    /// condenser end (`BubblePoint.vb:103-112`) the component-indexed specs and
    /// `Temperature` need the outer loop; everything else is direct. At the
    /// reboiler end (`:114-124`) `Stream_Ratio` joins them.
    /// **Result (2026-08-11):** classification matches upstream for all ten
    /// `SpecType` variants at both ends.
    #[test]
    fn spec_classification_matches_upstream_tables() {
        let outer_at_condenser = [
            SpecType::ComponentFraction,
            SpecType::ComponentMassFlowRate,
            SpecType::ComponentMolarFlowRate,
            SpecType::ComponentRecovery,
            SpecType::Temperature,
        ];
        let direct_at_condenser = [
            SpecType::HeatDuty,
            SpecType::ProductMolarFlowRate,
            SpecType::ProductMassFlowRate,
            SpecType::StreamRatio,
            SpecType::FeedRecovery,
        ];
        for st in outer_at_condenser {
            let s = ColumnSpec {
                spec_type: st,
                ..ColumnSpec::default()
            };
            assert!(!s.directly_imposable_at_condenser(), "{st:?}");
            assert!(!s.directly_imposable_at_reboiler(), "{st:?}");
        }
        for st in direct_at_condenser {
            let s = ColumnSpec {
                spec_type: st,
                ..ColumnSpec::default()
            };
            assert!(s.directly_imposable_at_condenser(), "{st:?}");
            // Stream_Ratio is the one that differs at the reboiler end.
            assert_eq!(
                s.directly_imposable_at_reboiler(),
                st != SpecType::StreamRatio,
                "{st:?}"
            );
        }
    }

    /// **Methodology.** `validate_shape` must accept a well-formed input and
    /// name the offending collection when one is short. Built on a minimal
    /// 3-stage / 2-component input.
    /// **Result (2026-08-11):** accepts the good case; returns
    /// `LengthMismatch { what: "vapor_flows", expected: 3, found: 2 }` when
    /// `vapor_flows` is truncated, and `TooFewStages` for a 1-stage column.
    #[test]
    fn shape_validation_catches_bad_lengths() {
        let mut input = minimal_input();
        assert!(input.validate_shape().is_ok());

        input.vapor_flows.pop();
        match input.validate_shape() {
            Err(ColumnError::LengthMismatch {
                what,
                expected,
                found,
            }) => {
                assert_eq!(what, "vapor_flows");
                assert_eq!((expected, found), (3, 2));
            }
            other => panic!("expected LengthMismatch, got {other:?}"),
        }

        let tiny = ColumnSolverInput {
            number_of_stages: 1,
            ..minimal_input()
        };
        assert!(matches!(
            tiny.validate_shape(),
            Err(ColumnError::TooFewStages { found: 1 })
        ));
    }

    /// **Methodology.** `uom` constructors must store the documented SI value:
    /// 101325 Pa, 353.15 K, 1 mol/s, 0.85 efficiency.
    /// **Result (2026-08-11):** all four round-trip to < 1e-12.
    #[test]
    fn uom_constructors_round_trip_to_si() {
        let stage = Stage::new(
            "top",
            StagePressure::new::<pascal>(101_325.0),
            StageTemperature::new::<kelvin>(353.15),
            2,
        )
        .with_feed(
            MolarFlowRate::new::<katal>(1.0),
            vec![0.5, 0.5],
            MolarEnthalpy::new::<joule_per_mole>(-1234.0),
        )
        .with_efficiency(StageEfficiency::new::<ratio>(0.85));

        assert!((stage.pressure - 101_325.0).abs() < 1e-12);
        assert!((stage.temperature - 353.15).abs() < 1e-12);
        assert!((stage.feed_molar_flow - 1.0).abs() < 1e-12);
        assert!((stage.efficiency - 0.85).abs() < 1e-12);
        assert!((stage.pressure().get::<pascal>() - 101_325.0).abs() < 1e-12);
    }

    fn minimal_input() -> ColumnSolverInput {
        use crate::thermo::component::reference;
        ColumnSolverInput {
            components: vec![reference::methane(), reference::ethane()],
            package: PropertyPackageModel::Ideal,
            number_of_stages: 3,
            max_iterations: 50,
            tolerances: vec![1e-5, 1e-5],
            early_stop_iteration: None,
            stage_temperatures: vec![200.0; 3],
            stage_pressures: vec![1e5; 3],
            stage_heats: vec![0.0; 3],
            stage_efficiencies: vec![1.0; 3],
            feed_flows: vec![0.0, 1.0, 0.0],
            feed_compositions: vec![vec![0.5, 0.5]; 3],
            feed_enthalpies: vec![0.0; 3],
            vapor_flows: vec![0.5; 3],
            vapor_compositions: vec![vec![0.5, 0.5]; 3],
            liquid_flows: vec![0.5; 3],
            liquid_compositions: vec![vec![0.5, 0.5]; 3],
            vapor_side_draws: vec![0.0; 3],
            liquid_side_draws: vec![0.0; 3],
            k_values: vec![vec![1.0, 1.0]; 3],
            overall_compositions: vec![vec![0.5, 0.5]; 3],
            condenser_type: CondenserType::TotalCondenser,
            column_type: ColumnType::DistillationColumn,
            condenser_spec: ColumnSpec::reflux_ratio(2.0),
            reboiler_spec: ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
            subcooling_delta_t: 0.0,
        }
    }
}
