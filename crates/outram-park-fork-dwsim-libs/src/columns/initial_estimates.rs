//! Column assembly and initial-estimate generation.
//!
//! Pure-Rust port of the flowsheet-**independent** core of DWSIM's
//! `Column.GetSolverInputData` — `RigorousColumn.vb` lines 2754-3706 (GPL-3.0),
//! upstream commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream
//! copyright: 2008-2022 Daniel Wagner O. de Medeiros et al.
//!
//! | This module | Upstream | `RigorousColumn.vb` lines |
//! |---|---|---|
//! | [`RigorousColumn::solver_input`] | `GetSolverInputData` (estimate-generation half) | 3280-3525 |
//! | [`estimate_temperature_profile`] | `T1`/`T2` + the linear ramp | 3283-3306, 3324-3336 |
//! | [`RigorousColumn::estimate_flows`] | `V(i)` / `L(i)` blocks | 3337-3421 |
//! | [`RigorousColumn::estimate_compositions`] | the `needsXYestimates` PT-flash block | 3422-3525 |
//!
//! # What this is for
//!
//! A rigorous MESH solve is a fixed-point iteration and needs a starting
//! profile. Upstream builds one from the flowsheet: it mixes the connected feed
//! streams, computes a bubble point at the top pressure and a dew point at the
//! bottom, ramps the temperature linearly between them, assumes constant molar
//! overflow for the flows, and PT-flashes the mixed feed on every stage for the
//! compositions and K-values.
//!
//! This port takes the same construction but reads the feeds from plain
//! per-stage [`Stage`] data instead of a flowsheet object graph — see
//! [`crate::columns::model`]'s "Excluded DWSIM behavior" for why.
//!
//! # Units
//!
//! `uom`-typed at the [`RigorousColumn`] boundary via [`Stage`]; the generated
//! [`ColumnSolverInput`] is documented raw `f64` in SI (\[K\], \[Pa\],
//! \[mol/s\], \[J/mol\], \[W\], \[-\]).
//!
//! # Excluded DWSIM behavior
//!
//! - The entire flowsheet-coupled half of `GetSolverInputData` (lines 2754-3279
//!   and 3526-3706): reading connected `MaterialStream`s, resolving side
//!   operations, `pp.CurrentMaterialStream` cloning, `Inspector` paragraphs,
//!   and the liquid-liquid extractor `L1trials` / `x1trials` seeding.
//! - `GetSolverInputData_New` (lines 3707-4726), the second, newer builder —
//!   it differs only in how it walks the flowsheet graph, which this port does
//!   not have.
//! - `UseTemperatureEstimates` / `UseVaporFlowEstimates` /
//!   `UseLiquidFlowEstimates` / `UseCompositionEstimates` as *flags*: this port
//!   uses whichever [`InitialEstimates`] fields are present and valid (the
//!   flags and the `Validate*` calls collapse into one check per profile).

use uom::si::catalytic_activity::katal;
use uom::si::power::watt;

use crate::columns::model::{
    ColumnError, ColumnSolverInput, ColumnSpec, ColumnType, CondenserType, InitialEstimates,
    MolarFlowRate, SolvingScheme, Stage, StageHeatDuty,
};
use crate::columns::thermo_bridge::ColumnThermo;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::{bubble_temperature, dew_temperature};
use crate::thermo::Component;

/// A rigorous MESH distillation / absorption column, ready to solve.
///
/// The human-facing assembly type: build the stage stack, attach feeds and side
/// draws, choose the two specifications, and call [`Self::solver_input`] to get
/// the [`ColumnSolverInput`] a solver consumes.
///
/// # Stage numbering
///
/// Top to bottom. Stage `0` is the condenser (when the column has one) and the
/// last stage is the reboiler (when it has one) — upstream's convention,
/// preserved (`RigorousColumn.vb:1919-1921`).
#[derive(Debug, Clone, PartialEq)]
pub struct RigorousColumn {
    /// Pure-component constants, length `n_components`.
    pub components: Vec<Component>,
    /// The thermodynamic model.
    pub package: PropertyPackageModel,
    /// The stage stack, top to bottom. At least 2 stages.
    pub stages: Vec<Stage>,
    /// Column configuration (distillation / absorption / reboiled or refluxed
    /// absorber).
    pub column_type: ColumnType,
    /// Condenser configuration.
    pub condenser_type: CondenserType,
    /// The condenser-end (upstream `"C"`) specification.
    pub condenser_spec: ColumnSpec,
    /// The reboiler-end (upstream `"R"`) specification.
    pub reboiler_spec: ColumnSpec,
    /// Inner-loop iteration budget (upstream default 100).
    pub max_iterations: usize,
    /// `[inner, outer]` convergence tolerances (upstream defaults `[1e-5, 1e-5]`).
    pub tolerances: Vec<f64>,
    /// Condenser sub-cooling \[K\] below the bubble point.
    pub subcooling_delta_t: f64,
    /// Initialisation strategy. Only [`SolvingScheme::Direct`] is implemented —
    /// see that enum's docs.
    pub solving_scheme: SolvingScheme,
    /// User-supplied starting profiles; whichever fields are present and valid
    /// override the generated estimate.
    pub initial_estimates: InitialEstimates,
    /// Reflux-ratio estimate `L_0 / D` \[-\] used to seed the internal flows
    /// (upstream's `rr`, default 5.0 — `RigorousColumn.vb:1926`).
    pub reflux_ratio_estimate: f64,
    /// Distillate molar-rate estimate \[mol/s\] (upstream's `distrate`).
    pub distillate_rate_estimate: f64,
    /// Overhead-vapour molar-rate estimate \[mol/s\] (upstream's `vaprate`);
    /// zero for a total condenser.
    pub vapor_rate_estimate: f64,
}

impl RigorousColumn {
    /// The shared body of the four constructors: upstream's defaults for the
    /// iteration budget (100), tolerances (`1e-5`), reflux-ratio seed (5.0), and
    /// a distillate-rate seed of half the total feed.
    fn with_configuration(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
        column_type: ColumnType,
        condenser_type: CondenserType,
        condenser_spec: ColumnSpec,
        reboiler_spec: ColumnSpec,
    ) -> Self {
        let total_feed: f64 = stages.iter().map(|s| s.feed_molar_flow).sum();
        Self {
            components,
            package,
            stages,
            column_type,
            condenser_type,
            condenser_spec,
            reboiler_spec,
            max_iterations: 100,
            tolerances: vec![1e-5, 1e-5],
            subcooling_delta_t: 0.0,
            solving_scheme: SolvingScheme::Direct,
            initial_estimates: InitialEstimates::default(),
            reflux_ratio_estimate: 5.0,
            distillate_rate_estimate: 0.5 * total_feed,
            vapor_rate_estimate: 0.0,
        }
    }

    /// A [`ColumnSpec::heat_duty`] carrying a stage's own `heat_duty` \[W\], so
    /// that a solver which reads an end duty from the spec sees the same number
    /// the caller put on the stage.
    fn heat_duty_spec_from_stage(stage: Option<&Stage>) -> ColumnSpec {
        let q = stage.map_or(0.0, |s| s.heat_duty);
        ColumnSpec::heat_duty(StageHeatDuty::new::<watt>(q))
    }

    /// A distillation column with `stages` stages, a total condenser, and the
    /// two given specifications.
    ///
    /// Iteration budget and tolerances take upstream's defaults (100 iterations,
    /// `1e-5`); the reflux-ratio seed is upstream's 5.0. The distillate-rate
    /// seed defaults to half the total feed, which is a neutral starting split.
    ///
    /// # Duty convention ([`ColumnType::DistillationColumn`])
    ///
    /// Condenser at stage `0` **and** reboiler at stage `n - 1`. **Both** end
    /// duties are back-calculated from the end-stage energy balances, unless
    /// the corresponding spec is a [`SpecType::HeatDuty`], in which case that
    /// duty is imposed and the balance solves for a flow instead. Whatever
    /// `heat_duty` the caller put on stage `0` or stage `n - 1` via
    /// [`Stage::with_heat_duty`] is **discarded** by [`Self::solver_input`]
    /// (zeroed before the solve); interior stage duties are honoured.
    ///
    /// [`SpecType::HeatDuty`]: crate::columns::model::SpecType::HeatDuty
    #[must_use]
    pub fn distillation(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
        condenser_spec: ColumnSpec,
        reboiler_spec: ColumnSpec,
    ) -> Self {
        Self::with_configuration(
            components,
            package,
            stages,
            ColumnType::DistillationColumn,
            CondenserType::TotalCondenser,
            condenser_spec,
            reboiler_spec,
        )
    }

    /// An absorption column ([`ColumnType::AbsorptionColumn`]): **no condenser,
    /// no reboiler**, and therefore no end specification at all.
    ///
    /// The lean solvent is fed to the top stage (index `0`) and the gas to the
    /// bottom stage (index `n - 1`) via [`Stage::with_feed`]. The overhead
    /// product is the vapour leaving stage `0` (`V_0`, read with
    /// [`ColumnSolverOutput::distillate_molar_flow`] and this column's
    /// [`Self::condenser_type`]); the bottoms product is the liquid leaving
    /// stage `n - 1` (`L_ns`, [`ColumnSolverOutput::bottoms_molar_flow`]). With
    /// feeds, pressures and stage count fixed the column has **zero** remaining
    /// degrees of freedom — every flow is set by the mass and energy balances.
    ///
    /// # Duty convention — both end duties are USER INPUT
    ///
    /// This is the rule that is silently got wrong, so it is spelled out:
    ///
    /// - **Nothing is back-calculated.** Unlike the other three column types,
    ///   the solvers never solve an end-stage energy balance for a duty
    ///   (`BubblePoint.vb:1664`, "use the provided values"). Every stage
    ///   including both ends is an ordinary adiabatic-or-specified stage.
    /// - **The end duties are read from the stages.** Whatever `heat_duty`
    ///   \[W\] the caller set on stage `0` and stage `n - 1` with
    ///   [`Stage::with_heat_duty`] is passed through unchanged by
    ///   [`Self::solver_input`] — [`Stage::new`]'s default of `0 W` gives an
    ///   adiabatic absorber, which is the normal case.
    /// - **The two specs are `HeatDuty` mirrors of those stages, not free
    ///   inputs.** This constructor sets [`Self::condenser_spec`] and
    ///   [`Self::reboiler_spec`] to [`ColumnSpec::heat_duty`] carrying stage
    ///   `0`'s and stage `n - 1`'s `heat_duty` respectively. It does this
    ///   because the ported solvers write the `HeatDuty` spec value **over**
    ///   the stage duty (`Q_0 = spec`, `BubblePoint.vb:987-990`;
    ///   `NewtonRaphson.vb:389-455`), so a spec left at its default `0 W` would
    ///   silently override a non-zero stage duty. Do not replace these specs
    ///   with a flow, ratio or purity spec: an absorber has no degree of
    ///   freedom for one to fix.
    /// - **Sign, for a non-zero end duty.** The two solvers disagree on the
    ///   sign of the bottom-end `HeatDuty` spec — Wang-Henke applies it as
    ///   `Q_ns = -value` (the `Heat_Duty` case of `BubblePoint.vb:997-1021`, faithfully ported), while
    ///   Naphtali-Sandholm applies `Q_ns = +value`. A non-zero bottom-stage
    ///   duty on an absorber therefore currently means opposite things to the
    ///   two solvers; `0 W` (adiabatic) is unambiguous and is the tested
    ///   configuration. This is a pre-existing solver-side inconsistency,
    ///   recorded here rather than hidden; the constructor does not attempt to
    ///   correct it.
    ///
    /// [`Self::condenser_type`] is [`CondenserType::FullReflux`], which is the
    /// configuration under which every solver in this port treats stage `0` as
    /// a plain stage with a vapour product and `LSS_0 = 0` — it does **not**
    /// mean anything is refluxed. Note that, as an internal-consistency
    /// convention, Wang-Henke's flow estimate seeds `V` from the bottom-most
    /// feed and `L` from the top-most feed (`RigorousColumn.vb:3337-3421`).
    ///
    /// # Which solver
    ///
    /// Use [`NaphtaliSandholmSolver`] (with or without warm start). Measured
    /// 2026-09-10 on a 6-stage benzene/toluene absorber (regression test
    /// `absorption_column_solves_through_the_public_api`): Naphtali-Sandholm
    /// converged in 7 iterations to `Σf² = 6.75e-7` with every energy balance
    /// closed; Wang-Henke and Modified Wang-Henke did **not** converge in 100
    /// iterations — the bubble-point family steps the vapour profile from a
    /// condenser-shaped stage `0` that this column does not have; sum-rates,
    /// upstream's own absorber method, failed with `InvalidProfile`, as it does
    /// on every column in this port (see [`crate::columns`]'s test module).
    ///
    /// # Example
    ///
    /// A 6-stage benzene/toluene absorber: toluene liquid (the "lean solvent")
    /// onto stage 0, saturated benzene-rich vapour onto stage 5, every stage
    /// adiabatic. The example checks the duty convention end to end: the specs
    /// mirror the stage duties, `solver_input` passes both end duties through
    /// unchanged, and `LSS_0` is not seeded with a distillate.
    ///
    /// ```
    /// use outram_park_fork_dwsim_libs::columns::initial_estimates::RigorousColumn;
    /// use outram_park_fork_dwsim_libs::columns::{
    ///     ColumnType, CondenserType, MolarEnthalpy, MolarFlowRate, SpecType, Stage,
    ///     StageHeatDuty, StagePressure, StageTemperature,
    /// };
    /// use outram_park_fork_dwsim_libs::thermo::component::reference::{benzene, toluene};
    /// use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
    /// use uom::si::catalytic_activity::katal;
    /// use uom::si::molar_energy::joule_per_mole;
    /// use uom::si::power::watt;
    /// use uom::si::pressure::pascal;
    /// use uom::si::thermodynamic_temperature::kelvin;
    ///
    /// let p = StagePressure::new::<pascal>(101_325.0);
    /// let mut stages: Vec<Stage> = (0..6)
    ///     .map(|i| {
    ///         let t = StageTemperature::new::<kelvin>(360.0 + 3.0 * i as f64);
    ///         Stage::new(format!("stage {i}"), p, t, 2)
    ///     })
    ///     .collect();
    /// // Lean toluene liquid on the top stage (enthalpy on the column's own
    /// // reference state; -26 kJ/mol is a saturated-liquid value at ~370 K).
    /// stages[0] = stages[0].clone().with_feed(
    ///     MolarFlowRate::new::<katal>(1.0),
    ///     vec![0.0, 1.0],
    ///     MolarEnthalpy::new::<joule_per_mole>(-26_000.0),
    /// );
    /// // Benzene-rich vapour on the bottom stage.
    /// stages[5] = stages[5].clone().with_feed(
    ///     MolarFlowRate::new::<katal>(1.0),
    ///     vec![0.9, 0.1],
    ///     MolarEnthalpy::new::<joule_per_mole>(6_000.0),
    /// );
    /// // An intercooler on the top stage: the ONLY way to put a duty on an
    /// // absorber's end stage is on the stage itself.
    /// stages[0] = stages[0].clone().with_heat_duty(StageHeatDuty::new::<watt>(-500.0));
    ///
    /// let column = RigorousColumn::absorption(
    ///     vec![benzene(), toluene()],
    ///     PropertyPackageModel::Ideal,
    ///     stages,
    /// );
    /// assert_eq!(column.column_type, ColumnType::AbsorptionColumn);
    /// assert_eq!(column.condenser_type, CondenserType::FullReflux);
    ///
    /// // Both specs are HeatDuty mirrors of the end stages -- not free inputs.
    /// assert_eq!(column.condenser_spec.spec_type, SpecType::HeatDuty);
    /// assert_eq!(column.condenser_spec.value, -500.0);
    /// assert_eq!(column.reboiler_spec.spec_type, SpecType::HeatDuty);
    /// assert_eq!(column.reboiler_spec.value, 0.0);
    ///
    /// // solver_input passes BOTH end duties through unchanged and seeds no
    /// // distillate draw on stage 0.
    /// let input = column.solver_input().unwrap();
    /// assert_eq!(input.stage_heats[0], -500.0);
    /// assert_eq!(input.stage_heats[5], 0.0);
    /// assert_eq!(input.liquid_side_draws[0], 0.0);
    /// ```
    ///
    /// [`ColumnSolverOutput::distillate_molar_flow`]: crate::columns::model::ColumnSolverOutput::distillate_molar_flow
    /// [`ColumnSolverOutput::bottoms_molar_flow`]: crate::columns::model::ColumnSolverOutput::bottoms_molar_flow
    /// [`NaphtaliSandholmSolver`]: crate::columns::newton_raphson::NaphtaliSandholmSolver
    #[must_use]
    pub fn absorption(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
    ) -> Self {
        let condenser_spec = Self::heat_duty_spec_from_stage(stages.first());
        let reboiler_spec = Self::heat_duty_spec_from_stage(stages.last());
        Self::with_configuration(
            components,
            package,
            stages,
            ColumnType::AbsorptionColumn,
            CondenserType::FullReflux,
            condenser_spec,
            reboiler_spec,
        )
    }

    /// A reboiled absorber ([`ColumnType::ReboiledAbsorber`]) — a **stripper**:
    /// reboiler at stage `n - 1`, **no condenser**, one specification (at the
    /// reboiler end).
    ///
    /// The liquid to be stripped is fed to the top stage (index `0`) via
    /// [`Stage::with_feed`]; the reboiler generates the stripping vapour. The
    /// overhead product is the vapour leaving stage `0` (`V_0`, read with
    /// [`ColumnSolverOutput::distillate_molar_flow`] and this column's
    /// [`Self::condenser_type`]); the bottoms product is `L_ns`. The column has
    /// **one** degree of freedom, fixed by `reboiler_spec` — a bottoms molar
    /// flow ([`ColumnSpec::product_molar_flow`]), a reboiler duty
    /// ([`ColumnSpec::heat_duty`]), a bottoms purity, or any other
    /// reboiler-end [`SpecType`].
    ///
    /// # Duty convention
    ///
    /// - **Reboiler duty `Q_ns` is back-calculated** from the bottom-stage
    ///   energy balance (the `ReboiledAbsorber` case of `BubblePoint.vb:1645-1673`) unless `reboiler_spec` is
    ///   a `HeatDuty`, in which case it is imposed. Any `heat_duty` the caller
    ///   set on stage `n - 1` is **discarded** by [`Self::solver_input`].
    /// - **Top-stage duty `Q_0` is user input**, read from stage `0`'s
    ///   `heat_duty` ([`Stage::with_heat_duty`]; default `0 W`, adiabatic) and
    ///   passed through unchanged. There is no condenser to back-calculate.
    /// - **There is no condenser-end specification.** The solvers force the
    ///   condenser-end spec to "directly imposable" and ignore it
    ///   (`BubblePoint.vb:126`, `:992-995`). This constructor sets
    ///   [`Self::condenser_spec`] to a `HeatDuty` mirror of stage `0`'s
    ///   `heat_duty`, so the one solver that still *reads* it
    ///   (Naphtali-Sandholm writes `Q_0 = spec`, `NewtonRaphson.vb:389-455`) sees
    ///   the same number as the stage. Leave it alone.
    ///
    /// [`Self::condenser_type`] is [`CondenserType::FullReflux`]: the
    /// configuration under which every solver treats stage `0` as a plain stage
    /// with a vapour product and `LSS_0 = 0`. It is required, not cosmetic —
    /// with a `TotalCondenser` the Naphtali-Sandholm residuals would zero `V_0`
    /// (`NewtonRaphson.vb:174-177`) and the stripper would have no overhead.
    ///
    /// # Example
    ///
    /// An 8-stage benzene/toluene stripper: equimolar saturated liquid onto
    /// stage 0, bottoms fixed at 0.5 mol/s. The example checks the duty
    /// convention: stage 7's duty is dropped (to be back-calculated), stage 0's
    /// is kept, the condenser-end spec mirrors stage 0, and no distillate draw
    /// is seeded.
    ///
    /// ```
    /// use outram_park_fork_dwsim_libs::columns::initial_estimates::RigorousColumn;
    /// use outram_park_fork_dwsim_libs::columns::{
    ///     ColumnSpec, ColumnType, CondenserType, MolarEnthalpy, MolarFlowRate, SpecType,
    ///     Stage, StageHeatDuty, StagePressure, StageTemperature,
    /// };
    /// use outram_park_fork_dwsim_libs::thermo::component::reference::{benzene, toluene};
    /// use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
    /// use uom::si::catalytic_activity::katal;
    /// use uom::si::molar_energy::joule_per_mole;
    /// use uom::si::power::watt;
    /// use uom::si::pressure::pascal;
    /// use uom::si::thermodynamic_temperature::kelvin;
    ///
    /// let p = StagePressure::new::<pascal>(101_325.0);
    /// let mut stages: Vec<Stage> = (0..8)
    ///     .map(|i| {
    ///         let t = StageTemperature::new::<kelvin>(360.0 + 3.0 * i as f64);
    ///         Stage::new(format!("stage {i}"), p, t, 2)
    ///     })
    ///     .collect();
    /// stages[0] = stages[0]
    ///     .clone()
    ///     .with_feed(
    ///         MolarFlowRate::new::<katal>(1.0),
    ///         vec![0.5, 0.5],
    ///         MolarEnthalpy::new::<joule_per_mole>(-25_000.0),
    ///     )
    ///     .with_heat_duty(StageHeatDuty::new::<watt>(-200.0)); // top-stage cooler: kept
    /// stages[7] = stages[7]
    ///     .clone()
    ///     .with_heat_duty(StageHeatDuty::new::<watt>(99_999.0)); // ignored: back-calculated
    ///
    /// let column = RigorousColumn::reboiled_absorber(
    ///     vec![benzene(), toluene()],
    ///     PropertyPackageModel::Ideal,
    ///     stages,
    ///     ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
    /// );
    /// assert_eq!(column.column_type, ColumnType::ReboiledAbsorber);
    /// assert_eq!(column.condenser_type, CondenserType::FullReflux);
    /// assert_eq!(column.reboiler_spec.spec_type, SpecType::ProductMolarFlowRate);
    /// // The condenser-end "spec" is a placeholder mirroring stage 0's duty.
    /// assert_eq!(column.condenser_spec.spec_type, SpecType::HeatDuty);
    /// assert_eq!(column.condenser_spec.value, -200.0);
    ///
    /// let input = column.solver_input().unwrap();
    /// assert_eq!(input.stage_heats[0], -200.0); // user input, kept
    /// assert_eq!(input.stage_heats[7], 0.0);    // reboiler: back-calculated
    /// assert_eq!(input.liquid_side_draws[0], 0.0);
    /// ```
    ///
    /// [`ColumnSolverOutput::distillate_molar_flow`]: crate::columns::model::ColumnSolverOutput::distillate_molar_flow
    /// [`SpecType`]: crate::columns::model::SpecType
    #[must_use]
    pub fn reboiled_absorber(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
        reboiler_spec: ColumnSpec,
    ) -> Self {
        let condenser_spec = Self::heat_duty_spec_from_stage(stages.first());
        Self::with_configuration(
            components,
            package,
            stages,
            ColumnType::ReboiledAbsorber,
            CondenserType::FullReflux,
            condenser_spec,
            reboiler_spec,
        )
    }

    /// A refluxed absorber ([`ColumnType::RefluxedAbsorber`]): total condenser
    /// at stage `0`, **no reboiler**, one specification (at the condenser end).
    ///
    /// This is the shape of a crude distillation unit (steam-stripped, no
    /// reboiler) and of a reformate stabiliser. The vapour to be rectified is
    /// fed to the bottom stage (index `n - 1`) via [`Stage::with_feed`] — since
    /// there is no reboiler, **the feed's own vapour fraction is the only
    /// source of up-flowing vapour**, so a saturated- or partly-vaporised feed
    /// is needed for the column to be well-posed. The distillate is the liquid
    /// draw off stage `0` (`LSS_0`, [`ColumnSolverOutput::distillate_molar_flow`]);
    /// the bottoms is `L_ns`. The column has **one** degree of freedom, fixed by
    /// `condenser_spec` — a reflux ratio ([`ColumnSpec::reflux_ratio`]), a
    /// distillate flow ([`ColumnSpec::product_molar_flow`]), a condenser duty, a
    /// distillate purity, or any other condenser-end [`SpecType`].
    ///
    /// # Duty convention
    ///
    /// - **Condenser duty `Q_0` is back-calculated** from the top-stage energy
    ///   balance unless `condenser_spec` is a `HeatDuty`, in which case it is
    ///   imposed. Any `heat_duty` the caller set on stage `0` is **discarded**
    ///   by [`Self::solver_input`].
    /// - **Bottom-stage duty `Q_ns` is also back-calculated** by the
    ///   bubble-point solvers, from the *overall* column energy balance.
    ///   Whatever `heat_duty` the caller put on stage `n - 1` is passed to the
    ///   solver but overwritten in the result. **Corrected 2026-09-11**: this
    ///   used to be passed through unchanged, which follows upstream's dead
    ///   `Case ColType.RefluxedAbsorber` arm rather than its live one — see
    ///   `back_calculate_duties` in [`crate::columns::bubble_point`] for the
    ///   two-mechanism upstream reading behind that.
    ///
    ///   A returned `Q_ns` is **not** a reboiler. It is the heat the bottom
    ///   stage would have to exchange for the returned profile to conserve
    ///   energy, in DWSIM's heat-*removed* sign — so a negative value means
    ///   heat would have to be **added**. On a genuinely adiabatic refluxed
    ///   absorber it should be zero; that it is not is the measure of how far
    ///   the bubble-point answer is from a solved column (see "Which solver"
    ///   below). Naphtali-Sandholm is different: it *imposes* `Q_ns` from the
    ///   reboiler spec (`NewtonRaphson.vb:480-481`) and keeps the bottom-stage
    ///   energy balance as an equation, which is why the spec below stays a
    ///   `HeatDuty`.
    /// - **There is no reboiler-end specification.** The solvers force the
    ///   reboiler-end spec to "directly imposable" and ignore it — the bottoms
    ///   rate is simply `B = L_ns` from the mass balance (`BubblePoint.vb:127`,
    ///   `:1020`). This constructor sets [`Self::reboiler_spec`] to a `HeatDuty`
    ///   mirror of stage `n - 1`'s `heat_duty` so that Naphtali-Sandholm, the
    ///   one solver that reads it, is told the bottom is adiabatic. Leave it
    ///   alone.
    ///
    /// # Which solver — read this before trusting the answer
    ///
    /// Measured 2026-09-10 on an 8-stage benzene/toluene case (regression test
    /// `refluxed_absorber_solves_through_the_public_api`, which records the
    /// full numbers):
    ///
    /// - **Wang-Henke / Modified Wang-Henke converge** (15 / 13 iterations),
    ///   close the mass balance, meet the condenser spec and back-calculate
    ///   both end duties — **but the distillate rate comes out exactly equal
    ///   to [`Self::with_distillate_estimate`]** (default: half the feed), and
    ///   the stage profile does not respond to the feed enthalpy at all.
    ///
    ///   This is **upstream's own limitation, not a porting slip**, and it is
    ///   structural. Upstream closes the bottom with `B = L_ns`
    ///   (`BubblePoint.vb:1020`) and `L_ns` with the total mass balance
    ///   (`:1625`), then re-derives `LSS_0` from the same balance (`:1553`);
    ///   composing the two returns `LSS_0` unchanged, so `D` is a fixed point
    ///   at its estimate. The bottom-stage energy balance — the equation that
    ///   would size the vapour the feed can raise — is not in Wang-Henke's
    ///   equation set: `Q_ns` enters `γ_ns`, which the vapour recursion never
    ///   reads (`:1596-1600`). And upstream deliberately removes the only
    ///   other handle, forcing `specR_OK = True` for a refluxed absorber
    ///   (`:127`) so the outer two-variable Broyden solve on
    ///   `(reflux ratio, bottoms rate)` never runs.
    ///
    ///   So what you get is a **mass-consistent rectifying section for the
    ///   `D` you asked for**, with the energy imbalance now reported honestly
    ///   as `Q_ns` (`−15 084 W` on the test case at the default estimate) —
    ///   not the adiabatic column's own `D`. Treat
    ///   [`Self::with_distillate_estimate`] as the second specification it
    ///   effectively is, and read a large `|Q_ns|` as "this `D` is wrong for
    ///   this feed".
    /// - **Naphtali-Sandholm solves it — use this one.** Converges in **3**
    ///   iterations on the test case and returns `D = 0.348081 mol/s` (not
    ///   the `0.5` estimate), `Q_ns = 0` genuinely enforced, and a distillate
    ///   that responds to the feed: half-vaporising the feed drops `D` to
    ///   `0.182524 mol/s` and cools every stage. This is the solver whose
    ///   equation set actually contains the bottom-stage energy balance —
    ///   upstream remaps the column type at `NewtonRaphson.vb:837-840` and
    ///   then replaces **only** `H(0)` with the condenser-spec residual
    ///   (`:546-556`), leaving stage `ns`'s true energy balance with
    ///   `Q_ns = spec` in the system (`:480-481`).
    ///
    ///   **Fixed 2026-09-11.** It previously returned `NotConverged` from any
    ///   starting profile. Two upstream gates keyed on `ColumnType` — the
    ///   stage-0 initial-estimate seeding (`NewtonRaphson.vb:969`) and the
    ///   stage-0 output mapping (`:1224`, `:1242`) — excluded the variant,
    ///   leaving `v_0` at `1e-10` and forcing `LSS_0 = 0`. Neither is part of
    ///   the residual system, so widening them changes the starting point and
    ///   the reporting, not the physics; see
    ///   [`has_condenser_distillate`](crate::columns::newton_raphson) in
    ///   `newton_raphson` for the full argument, the measurements, and the
    ///   cross-check against Wang-Henke.
    ///
    /// **How to read a bubble-point answer against this.** Wang-Henke's
    /// back-calculated `Q_ns` is the bottom-stage energy deficit at the `D`
    /// you chose, and it passes through zero at exactly the `D`
    /// Naphtali-Sandholm returns (`−15 083.87 W` at `D = 0.5`, `−0.02 W` at
    /// `D = 0.348081`, with the two temperature profiles then agreeing to
    /// `0.001 K`). So a large `|Q_ns|` from a bubble-point run means "this
    /// `D` is wrong for this feed", and the bubble-point family can be used
    /// as a check on, or a search over, `D` — but not as a solver for it.
    ///
    /// **None of this is validated against DWSIM or a published case.** It is
    /// verification only: two independent algorithms in this port agreeing on
    /// the same MESH solution.
    ///
    /// # Example
    ///
    /// An 8-stage benzene/toluene refluxed absorber: equimolar saturated vapour
    /// onto stage 7, reflux ratio 2. The example checks the duty convention:
    /// stage 0's duty is dropped (to be back-calculated), stage 7's is kept, and
    /// the reboiler-end spec mirrors stage 7.
    ///
    /// ```
    /// use outram_park_fork_dwsim_libs::columns::initial_estimates::RigorousColumn;
    /// use outram_park_fork_dwsim_libs::columns::{
    ///     ColumnSpec, ColumnType, CondenserType, MolarEnthalpy, MolarFlowRate, SpecType,
    ///     Stage, StageHeatDuty, StagePressure, StageTemperature,
    /// };
    /// use outram_park_fork_dwsim_libs::thermo::component::reference::{benzene, toluene};
    /// use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
    /// use uom::si::catalytic_activity::katal;
    /// use uom::si::molar_energy::joule_per_mole;
    /// use uom::si::power::watt;
    /// use uom::si::pressure::pascal;
    /// use uom::si::thermodynamic_temperature::kelvin;
    ///
    /// let p = StagePressure::new::<pascal>(101_325.0);
    /// let mut stages: Vec<Stage> = (0..8)
    ///     .map(|i| {
    ///         let t = StageTemperature::new::<kelvin>(355.0 + 3.0 * i as f64);
    ///         Stage::new(format!("stage {i}"), p, t, 2)
    ///     })
    ///     .collect();
    /// stages[0] = stages[0]
    ///     .clone()
    ///     .with_heat_duty(StageHeatDuty::new::<watt>(99_999.0)); // ignored: back-calculated
    /// stages[7] = stages[7]
    ///     .clone()
    ///     .with_feed(
    ///         MolarFlowRate::new::<katal>(1.0),
    ///         vec![0.5, 0.5],
    ///         MolarEnthalpy::new::<joule_per_mole>(7_000.0), // saturated vapour
    ///     )
    ///     .with_heat_duty(StageHeatDuty::new::<watt>(300.0)); // bottom-stage duty: kept
    ///
    /// let column = RigorousColumn::refluxed_absorber(
    ///     vec![benzene(), toluene()],
    ///     PropertyPackageModel::Ideal,
    ///     stages,
    ///     ColumnSpec::reflux_ratio(2.0),
    /// );
    /// assert_eq!(column.column_type, ColumnType::RefluxedAbsorber);
    /// assert_eq!(column.condenser_type, CondenserType::TotalCondenser);
    /// assert_eq!(column.condenser_spec.spec_type, SpecType::StreamRatio);
    /// // The reboiler-end "spec" is a placeholder mirroring stage 7's duty.
    /// assert_eq!(column.reboiler_spec.spec_type, SpecType::HeatDuty);
    /// assert_eq!(column.reboiler_spec.value, 300.0);
    ///
    /// let input = column.solver_input().unwrap();
    /// assert_eq!(input.stage_heats[0], 0.0);   // condenser: back-calculated
    /// assert_eq!(input.stage_heats[7], 300.0); // user input, kept
    /// // A total condenser seeds the distillate draw on stage 0.
    /// assert!(input.liquid_side_draws[0] > 0.0);
    /// ```
    ///
    /// [`ColumnSolverOutput::distillate_molar_flow`]: crate::columns::model::ColumnSolverOutput::distillate_molar_flow
    /// [`SpecType`]: crate::columns::model::SpecType
    /// [`NaphtaliSandholmSolver`]: crate::columns::newton_raphson::NaphtaliSandholmSolver
    #[must_use]
    pub fn refluxed_absorber(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
        condenser_spec: ColumnSpec,
    ) -> Self {
        let reboiler_spec = Self::heat_duty_spec_from_stage(stages.last());
        Self::with_configuration(
            components,
            package,
            stages,
            ColumnType::RefluxedAbsorber,
            CondenserType::TotalCondenser,
            condenser_spec,
            reboiler_spec,
        )
    }

    /// Set the distillate molar-rate estimate \[mol/s\].
    #[must_use]
    pub fn with_distillate_estimate(mut self, rate: MolarFlowRate) -> Self {
        self.distillate_rate_estimate = rate.get::<katal>();
        self
    }

    /// Set the reflux-ratio estimate `L_0 / D` \[-\].
    #[must_use]
    pub fn with_reflux_ratio_estimate(mut self, rr: f64) -> Self {
        self.reflux_ratio_estimate = rr;
        self
    }

    /// Number of components.
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.components.len()
    }

    /// Number of stages.
    #[must_use]
    pub fn n_stages(&self) -> usize {
        self.stages.len()
    }

    /// Total molar feed rate \[mol/s\].
    #[must_use]
    pub fn total_feed(&self) -> f64 {
        self.stages.iter().map(|s| s.feed_molar_flow).sum()
    }

    /// Mixed overall feed composition `z_m` \[-\] — upstream's `zm`, the
    /// flow-weighted average of every stage feed (`RigorousColumn.vb:3234` area).
    ///
    /// Returns a uniform composition if there is no feed at all, so downstream
    /// flashes stay well-posed.
    #[must_use]
    pub fn mixed_feed_composition(&self) -> Vec<f64> {
        let nc = self.n_components();
        let total = self.total_feed();
        if total <= 0.0 {
            return vec![1.0 / nc as f64; nc];
        }
        let mut z = vec![0.0_f64; nc];
        for s in &self.stages {
            for j in 0..nc {
                z[j] += s.feed_molar_flow * s.feed_composition.get(j).copied().unwrap_or(0.0);
            }
        }
        for v in z.iter_mut() {
            *v /= total;
        }
        let s: f64 = z.iter().sum();
        if s > 0.0 {
            for v in z.iter_mut() {
                *v /= s;
            }
        }
        z
    }

    /// Build the [`ColumnSolverInput`] a solver consumes.
    ///
    /// Generates whatever the caller did not supply in
    /// [`Self::initial_estimates`]: the temperature ramp
    /// ([`estimate_temperature_profile`]), the internal flows
    /// ([`Self::estimate_flows`]), and the compositions and K-values
    /// ([`Self::estimate_compositions`]).
    ///
    /// # Errors
    ///
    /// - [`ColumnError::TooFewStages`] for fewer than 2 stages.
    /// - [`ColumnError::LengthMismatch`] if a stage's feed composition does not
    ///   have `n_components` entries.
    /// - [`ColumnError::InvalidSpec`] if a spec's component index is out of
    ///   range.
    /// - [`ColumnError::BubblePointFailed`] if neither a bubble point at the top
    ///   pressure nor a dew point at the bottom pressure can be found and no
    ///   user temperature estimate was supplied.
    pub fn solver_input(&self) -> Result<ColumnSolverInput, ColumnError> {
        let n = self.n_stages();
        let nc = self.n_components();
        if n < 2 {
            return Err(ColumnError::TooFewStages { found: n });
        }
        for (i, s) in self.stages.iter().enumerate() {
            if s.feed_composition.len() != nc {
                return Err(ColumnError::LengthMismatch {
                    what: "stage feed_composition",
                    expected: nc,
                    found: s.feed_composition.len(),
                });
            }
            let _ = i;
        }

        let thermo = ColumnThermo::new(self.components.clone(), self.package);
        let zm = self.mixed_feed_composition();

        let stage_pressures: Vec<f64> = self.stages.iter().map(|s| s.pressure).collect();
        let stage_efficiencies: Vec<f64> = self.stages.iter().map(|s| s.efficiency).collect();
        let feed_flows: Vec<f64> = self.stages.iter().map(|s| s.feed_molar_flow).collect();
        let feed_compositions: Vec<Vec<f64>> = self
            .stages
            .iter()
            .map(|s| s.feed_composition.clone())
            .collect();
        let feed_enthalpies: Vec<f64> = self.stages.iter().map(|s| s.feed_molar_enthalpy).collect();
        let vapor_side_draws: Vec<f64> = self.stages.iter().map(|s| s.vapor_side_draw).collect();

        // Temperatures.
        let stage_temperatures = if self.initial_estimates.temperatures_valid()
            && self.initial_estimates.stage_temperatures.len() == n
        {
            self.initial_estimates.stage_temperatures.clone()
        } else {
            estimate_temperature_profile(
                &self.components,
                self.package,
                &zm,
                &stage_pressures,
                self.stages.first().map(|s| s.temperature),
                self.stages.last().map(|s| s.temperature),
            )?
        };

        // Flows.
        let (vapor_flows, liquid_flows, mut liquid_side_draws) =
            self.estimate_flows(&feed_flows, &vapor_side_draws);

        // Distillate appears as the stage-0 liquid side draw for a
        // total/partial condenser (upstream lines 3510-3512).
        if self.column_type != ColumnType::AbsorptionColumn
            && self.condenser_type != CondenserType::FullReflux
        {
            liquid_side_draws[0] = self.distillate_rate_estimate;
        }

        // Compositions and K-values.
        let (liquid_compositions, vapor_compositions, k_values) =
            self.estimate_compositions(&thermo, &zm, &stage_temperatures, &stage_pressures);

        let mut stage_heats: Vec<f64> = self.stages.iter().map(|s| s.heat_duty).collect();
        match self.column_type {
            ColumnType::DistillationColumn => {
                stage_heats[0] = 0.0;
                stage_heats[n - 1] = 0.0;
            }
            ColumnType::ReboiledAbsorber => stage_heats[n - 1] = 0.0,
            ColumnType::RefluxedAbsorber => stage_heats[0] = 0.0,
            ColumnType::AbsorptionColumn => {}
        }

        let input = ColumnSolverInput {
            components: self.components.clone(),
            package: self.package,
            number_of_stages: n,
            max_iterations: self.max_iterations,
            tolerances: self.tolerances.clone(),
            early_stop_iteration: None,
            stage_temperatures,
            stage_pressures,
            stage_heats,
            stage_efficiencies,
            feed_flows,
            feed_compositions,
            feed_enthalpies,
            vapor_flows,
            vapor_compositions,
            liquid_flows,
            liquid_compositions,
            vapor_side_draws,
            liquid_side_draws,
            k_values,
            overall_compositions: vec![zm; n],
            condenser_type: self.condenser_type,
            column_type: self.column_type,
            condenser_spec: self.condenser_spec.clone(),
            reboiler_spec: self.reboiler_spec.clone(),
            subcooling_delta_t: self.subcooling_delta_t,
        };
        input.validate_shape()?;
        Ok(input)
    }

    /// Constant-molar-overflow flow estimates — upstream's `V(i)` / `L(i)`
    /// blocks (`RigorousColumn.vb:3337-3421`).
    ///
    /// Returns `(vapor_flows, liquid_flows, liquid_side_draws)`, all \[mol/s\].
    ///
    /// For a distillation column with a total condenser: `V_0 = 1e-10` (nothing
    /// leaves the top as vapour), `V_i = (R + 1) D − F_0` for `i > 0`,
    /// `L_0 = R D`, and `L_i` from the running total mass balance
    /// `L_i = V_{i+1} + Σ_{m<=i}(F − U − W) − V_0`. Partial condensers add the
    /// overhead vapour rate to `D`; full reflux drives everything off `V_0`.
    /// An absorber simply propagates the end feeds.
    ///
    /// **Refluxed absorber (this port, 2026-09-10):** the distillate estimate
    /// is counted in the running sum as the stage-0 liquid draw it becomes, so
    /// `L_ns = ΣF − D`. Upstream leaves `D` out for every type; that is
    /// harmless where a spec re-imposes the end flows on the first pass, but a
    /// refluxed absorber reads `B = L_ns` from this estimate and the
    /// inconsistency pinned the bubble-point solvers to the all-liquid `D = 0`
    /// solution — see [`Self::refluxed_absorber`].
    #[must_use]
    pub fn estimate_flows(
        &self,
        feed_flows: &[f64],
        vapor_side_draws: &[f64],
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = self.n_stages();
        let ns = n - 1;
        let rr = self.reflux_ratio_estimate;
        let distrate = self.distillate_rate_estimate;
        let vaprate = self.vapor_rate_estimate;

        let liquid_side_draws: Vec<f64> = self.stages.iter().map(|s| s.liquid_side_draw).collect();

        let use_v = self.initial_estimates.vapor_flows_valid()
            && self.initial_estimates.vapor_molar_flows.len() == n;
        let use_l = self.initial_estimates.liquid_flows_valid()
            && self.initial_estimates.liquid_molar_flows.len() == n;

        // Running net feed sum: sum1_i = Σ_{m<=i} (F_m − U_m − W_m).
        //
        // For a refluxed absorber with a liquid distillate the distillate
        // estimate is counted as the stage-0 liquid draw it will become
        // (`solver_input` writes `LSS_0 = distillate_rate_estimate` after this
        // call), so that `L_ns = F − D` closes the overall balance. Every other
        // column type re-imposes its end flows from a spec on the first solver
        // pass, so leaving `D` out of `sum1` — as upstream does — is harmless
        // there; a refluxed absorber has no reboiler spec and reads
        // `B = L_ns` straight from this estimate (`BubblePoint.vb:1020`), so
        // an `L_ns` that ignores `D` pins the bubble-point solvers to the
        // degenerate all-liquid solution `D = 0` (measured 2026-09-10; see
        // `refluxed_absorber_solves_through_the_public_api`).
        let counts_distillate_in_sum1 = self.column_type == ColumnType::RefluxedAbsorber
            && self.condenser_type != CondenserType::FullReflux;
        let mut sum1 = vec![0.0_f64; n];
        let mut running = 0.0_f64;
        for i in 0..n {
            running += feed_flows[i] - liquid_side_draws[i] - vapor_side_draws[i];
            if i == 0 && counts_distillate_in_sum1 {
                running -= distrate;
            }
            sum1[i] = running;
        }

        let first_feed = feed_flows.iter().position(|f| *f > 0.0).unwrap_or(0);
        let last_feed = feed_flows.iter().rposition(|f| *f > 0.0).unwrap_or(n - 1);

        let mut v = vec![0.0_f64; n];
        let mut l = vec![0.0_f64; n];

        if use_v {
            v.clone_from(&self.initial_estimates.vapor_molar_flows);
        } else {
            match self.column_type {
                ColumnType::DistillationColumn | ColumnType::RefluxedAbsorber => {
                    v[0] = if self.condenser_type == CondenserType::TotalCondenser {
                        1.0e-10
                    } else {
                        vaprate
                    };
                    for i in 1..n {
                        v[i] = match self.condenser_type {
                            CondenserType::PartialCondenser => {
                                (rr + 1.0) * (distrate + vaprate) - feed_flows[0]
                            }
                            CondenserType::FullReflux => (rr + 1.0) * v[0] - feed_flows[0],
                            CondenserType::TotalCondenser => (rr + 1.0) * distrate - feed_flows[0],
                        };
                        if self.column_type == ColumnType::RefluxedAbsorber {
                            v[i] += v[0];
                        }
                    }
                }
                ColumnType::AbsorptionColumn | ColumnType::ReboiledAbsorber => {
                    let vf = feed_flows[last_feed];
                    for vi in v.iter_mut() {
                        *vi = vf;
                    }
                }
            }
        }

        if use_l {
            l.clone_from(&self.initial_estimates.liquid_molar_flows);
        } else {
            match self.column_type {
                ColumnType::DistillationColumn | ColumnType::RefluxedAbsorber => {
                    l[0] = match self.condenser_type {
                        CondenserType::PartialCondenser => (distrate + vaprate) * rr,
                        CondenserType::FullReflux => vaprate.max(v[0]) * rr,
                        CondenserType::TotalCondenser => distrate * rr,
                    };
                    for i in 1..n {
                        l[i] = if i < ns {
                            v[i] + sum1[i] - v[0]
                        } else {
                            sum1[i] - v[0]
                        };
                    }
                }
                ColumnType::AbsorptionColumn | ColumnType::ReboiledAbsorber => {
                    let lf = feed_flows[first_feed];
                    for li in l.iter_mut() {
                        *li = lf;
                    }
                }
            }
        }

        for x in v.iter_mut().chain(l.iter_mut()) {
            if !x.is_finite() || *x <= 0.0 {
                *x = 1.0e-5;
            }
        }

        (v, l, liquid_side_draws)
    }

    /// Per-stage composition and K-value estimates — upstream's
    /// `needsXYestimates` block (`RigorousColumn.vb:3500-3524`).
    ///
    /// Runs an isothermal-isobaric flash of the **mixed feed** `z_m` at every
    /// stage's `(P_j, T_j)` and takes the resulting `(x, y, K)`. Where the flash
    /// fails or returns a single phase, falls back to the ideal K-relation
    /// `x_i = z_i (L + V) / (L + V K_i)`, `y_i = K_i x_i` — which is upstream's
    /// absorption-column branch (lines 3562-3568).
    ///
    /// Returns `(liquid_compositions, vapor_compositions, k_values)`, all \[-\]
    /// and shaped `[stage][component]`.
    #[must_use]
    pub fn estimate_compositions(
        &self,
        thermo: &ColumnThermo,
        zm: &[f64],
        stage_temperatures: &[f64],
        stage_pressures: &[f64],
    ) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<Vec<f64>>) {
        let n = self.n_stages();
        let nc = self.n_components();

        if self.initial_estimates.compositions_valid()
            && self.initial_estimates.liquid_compositions.len() == n
            && self.initial_estimates.vapor_compositions.len() == n
        {
            let x = self.initial_estimates.liquid_compositions.clone();
            let y = self.initial_estimates.vapor_compositions.clone();
            let k = (0..n)
                .map(|i| thermo.k_values(&x[i], &y[i], stage_temperatures[i], stage_pressures[i]))
                .collect();
            return (x, y, k);
        }

        let mut x = vec![vec![0.0_f64; nc]; n];
        let mut y = vec![vec![0.0_f64; nc]; n];
        let mut k = vec![vec![1.0_f64; nc]; n];

        for i in 0..n {
            let t = stage_temperatures[i];
            let p = stage_pressures[i];
            let flashed = self.package.flash_pt(&self.components, zm, t, p).ok();
            match flashed {
                Some(fr) if fr.beta > 1.0e-9 && fr.beta < 1.0 - 1.0e-9 => {
                    x[i].clone_from(&fr.x);
                    y[i].clone_from(&fr.y);
                    k[i].clone_from(&fr.k);
                }
                _ => {
                    // Ideal-K split of the mixed feed (upstream's absorber
                    // branch): x_i = z_i (L+V) / (L + V K_i), y_i = K_i x_i.
                    let ki = thermo.wilson_k(t, p);
                    let mut sx = 0.0;
                    let mut sy = 0.0;
                    for j in 0..nc {
                        let denom = 1.0 + ki[j];
                        x[i][j] = if denom > 0.0 {
                            2.0 * zm[j] / denom
                        } else {
                            zm[j]
                        };
                        y[i][j] = ki[j] * x[i][j];
                        sx += x[i][j];
                        sy += y[i][j];
                    }
                    if sx > 0.0 {
                        for j in 0..nc {
                            x[i][j] /= sx;
                        }
                    }
                    if sy > 0.0 {
                        for j in 0..nc {
                            y[i][j] /= sy;
                        }
                    }
                    k[i] = ki;
                }
            }
        }
        (x, y, k)
    }
}

/// The linear temperature ramp between a top bubble point and a bottom dew
/// point — upstream's `T1`/`T2` and `T(i) = (T2 − T1) i/ns + T1`
/// (`RigorousColumn.vb:3288`, `:3303`, `:3336`).
///
/// `T1` is the **bubble** temperature of the mixed feed at the top-stage
/// pressure (the coldest the column top can be while still condensing) and `T2`
/// the **dew** temperature at the bottom-stage pressure. Both may be overridden
/// by `top_override` / `bottom_override`, which upstream uses when the
/// corresponding end spec is a [`crate::columns::model::SpecType::Temperature`].
///
/// # Parameters
///
/// - `components` / `package` — the thermodynamic model.
/// - `zm` — mixed feed mole fractions \[-\].
/// - `pressures` — per-stage pressures \[Pa\], length >= 2.
/// - `top_override` / `bottom_override` — end temperatures \[K\] to use instead
///   of the computed saturation points; ignored when `None` or non-positive.
///
/// # Returns
///
/// The per-stage temperature estimate \[K\], length `pressures.len()`.
///
/// # Errors
///
/// [`ColumnError::BubblePointFailed`] if the saturation calculation fails and
/// no override was supplied.
pub fn estimate_temperature_profile(
    components: &[Component],
    package: PropertyPackageModel,
    zm: &[f64],
    pressures: &[f64],
    top_override: Option<f64>,
    bottom_override: Option<f64>,
) -> Result<Vec<f64>, ColumnError> {
    let n = pressures.len();
    if n < 2 {
        return Err(ColumnError::TooFewStages { found: n });
    }
    let ns = n - 1;

    let t1 = match top_override.filter(|t| t.is_finite() && *t > 0.0) {
        Some(t) => t,
        None => bubble_temperature(components, zm, pressures[0], package)
            .map(|s| s.temperature)
            .map_err(|e| ColumnError::BubblePointFailed {
                stage: 0,
                pressure: pressures[0],
                detail: format!("top-stage bubble point: {e}"),
            })?,
    };
    let t2 = match bottom_override.filter(|t| t.is_finite() && *t > 0.0) {
        Some(t) => t,
        None => dew_temperature(components, zm, pressures[ns], package)
            .map(|s| s.temperature)
            .map_err(|e| ColumnError::BubblePointFailed {
                stage: ns,
                pressure: pressures[ns],
                detail: format!("bottom-stage dew point: {e}"),
            })?,
    };

    Ok((0..n)
        .map(|i| (t2 - t1) * (i as f64) / (ns as f64) + t1)
        .collect())
}
