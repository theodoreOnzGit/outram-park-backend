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

use crate::columns::model::{
    ColumnError, ColumnSolverInput, ColumnSpec, ColumnType, CondenserType, InitialEstimates,
    MolarFlowRate, SolvingScheme, Stage,
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
    /// A distillation column with `stages` stages, a total condenser, and the
    /// two given specifications.
    ///
    /// Iteration budget and tolerances take upstream's defaults (100 iterations,
    /// `1e-5`); the reflux-ratio seed is upstream's 5.0. The distillate-rate
    /// seed defaults to half the total feed, which is a neutral starting split.
    #[must_use]
    pub fn distillation(
        components: Vec<Component>,
        package: PropertyPackageModel,
        stages: Vec<Stage>,
        condenser_spec: ColumnSpec,
        reboiler_spec: ColumnSpec,
    ) -> Self {
        let total_feed: f64 = stages.iter().map(|s| s.feed_molar_flow).sum();
        Self {
            components,
            package,
            stages,
            column_type: ColumnType::DistillationColumn,
            condenser_type: CondenserType::TotalCondenser,
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
        let mut sum1 = vec![0.0_f64; n];
        let mut running = 0.0_f64;
        for i in 0..n {
            running += feed_flows[i] - liquid_side_draws[i] - vapor_side_draws[i];
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
