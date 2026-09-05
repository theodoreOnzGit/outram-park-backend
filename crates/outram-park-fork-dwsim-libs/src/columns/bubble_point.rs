//! Wang-Henke bubble-point (BP) method for the rigorous MESH column.
//!
//! Pure-Rust port of DWSIM's
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumnSolvers/BubblePoint.vb`
//! (GPL-3.0), class `WangHenkeMethod`, upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al.
//!
//! | This module | Upstream member | `BubblePoint.vb` lines |
//! |---|---|---|
//! | [`WangHenkeSolver::solve`] | `Public Function Solve` (outer spec loop) | 51-761 |
//! | [`WangHenkeSolver::solve_internal`] | `Public Function Solve_Internal` | 763-1857 |
//! | [`WangHenkeSolver::solve_column`] | `Public Overrides Function SolveColumn` | 1859-2020 |
//!
//! # Method provenance
//!
//! Wang, J. C. & Henke, G. E. (1966), "Tridiagonal matrix for distillation",
//! *Hydrocarbon Processing* **45**(8), 155-163. The MESH equations are torn on
//! the stage temperatures `T_j` and vapour flows `V_j`:
//!
//! 1. **M** — with `T` and `V` held, the component mass balances are linear in
//!    the component liquid flows and solve as one tridiagonal system per
//!    component ([`crate::columns::tridiagonal`]).
//! 2. **S/E** — new stage temperatures come from a **bubble-point** calculation
//!    on the normalised liquid `x_j` at the stage pressure, which is what gives
//!    the method its name; the vapour is then `y_j = K_j x_j` (corrected for
//!    stage efficiency).
//! 3. **H** — the energy balances, now linear in `V`, are solved by forward
//!    substitution from the top: `V_{j+1} = (γ_j − α_j V_j)/β_j` with
//!    `α_j = H^L_{j-1} − H^V_j`, `β_j = H^V_{j+1} − H^L_j`, and `γ_j` collecting
//!    the feed, side-draw and duty terms. Liquid flows follow from the total
//!    mass balance.
//!
//! The method is reliable for **narrow-boiling** mixtures (ordinary
//! distillation). For wide-boiling mixtures the bubble-point temperature is too
//! sensitive to `x` and the method stalls — upstream detects this
//! (`K_max/K_min > 10000`, line 1305) and switches the temperature update to
//! [`TemperatureUpdate::BroydenOnSummation`]; for genuinely wide-boiling
//! absorbers, use [`crate::columns::sum_rates`] instead.
//!
//! # Two-level iteration
//!
//! - The **inner** loop ([`WangHenkeSolver::solve_internal`]) converges the
//!   profile for a *reflux-ratio-and-bottoms-rate* pair of specifications,
//!   which it can impose directly on the mass balance.
//! - The **outer** loop ([`WangHenkeSolver::solve`]) exists only when the user's
//!   specifications are of a kind the inner loop cannot impose directly
//!   (a product purity, a component recovery, an end-stage temperature). It
//!   root-finds on the reflux ratio and/or bottoms rate until the user's real
//!   specifications are met, by Broyden's method
//!   ([`crate::columns::linalg::broyden_root`]).
//!
//! # Units
//!
//! Documented raw `f64` in SI: `T` \[K\], `P` \[Pa\], flows \[mol/s\], molar
//! enthalpies \[J/mol\], duties \[W\], compositions and K-values \[-\].
//!
//! # Excluded DWSIM behavior
//!
//! - **`Inspector` trace paragraphs** and the `ColumnSolverConvergenceReport`
//!   `StringBuilder` (lines 779-785, 1039-1084, 1682-1699, 1738-1787,
//!   1804-1817, 1838-1851). Pure HTML/text reporting, no numerics.
//! - **`Parallel.For` branches** (lines 949-969, 1253-1273, 1371-1392,
//!   1461-1484). Evaluated serially here; the arithmetic is identical.
//! - **`pp.Flowsheet?.ShowMessage` / `CheckStatus` / `Calculator.WriteToConsole`**
//!   (lines 302, 330, 501, 515, 656, 963, 1279, 1734) — flowsheet UI messaging
//!   and cooperative cancellation.
//! - **`AbsorptionColumn.OperationMode = Extractor`** (the `llextractor` flag,
//!   lines 1890-1898) and the `L1trials`/`x1trials` liquid-liquid seeds — this
//!   port has no liquid-liquid extractor mode.
//! - **`SystemsOfUnits.Converter.ConvertToSI`** on spec values (lines 88, 97,
//!   886, 898): spec values arrive in SI (see [`crate::columns::specs`]).
//! - **The `Brent` / `BrentOpt2` fallbacks** for the reboiler-only outer loop
//!   (lines 709-733). This port uses one root finder
//!   ([`crate::columns::linalg::broyden_root`]) for every outer-loop shape,
//!   with the best-point-visited semantics upstream gets from its
//!   `ObjFunctionValues.IndexOf(Min)` bookkeeping. A three-way
//!   Broyden/Brent-optimise/Brent-expand cascade is a workaround for MathNet's
//!   Broyden being fragile, not physics.
//! - **The `Mode 0`/`Mode 1` retry-on-exception cascade** (lines 357-370,
//!   393-406, 532-545, 746-757): upstream re-runs the whole inner loop in the
//!   Broyden temperature mode whenever the bubble-point mode throws. This port
//!   keeps the *mode* (see [`TemperatureUpdate`]) and the automatic
//!   wide-boiling switch at line 1305, but does not retry a failed solve — a
//!   failure is returned to the caller as a typed [`ColumnError`].
//!
//! # Faithfully ported upstream quirks (documented, not "fixed")
//!
//! - The `Mode 1` residual is built from the **input** liquid composition `x`,
//!   not the current iterate `xc` (line 1331, `fx(i) = 1 - K(i).MultiplyY(x(i)).SumY()`).
//!   `x` is never written inside `Solve_Internal`, so this residual is
//!   evaluated at a frozen composition. It is preserved here because changing it
//!   changes which fixed point the wide-boiling branch converges to, and this
//!   port makes no claim to improve on upstream's numerics.
//! - The temperature damping in `Mode 0` reads `If maxDT < 50`, but `maxDT` is
//!   initialised to exactly `50.0` and only ever lowered inside the `Mode 1`
//!   branch (lines 1089, 1312, 1354). On a pure `Mode 0` solve the test is
//!   therefore always false and the update is the unconditional 50 % average
//!   `T = (T_new + T_old)/2`. Preserved.

use ndarray::Array2;

use crate::columns::linalg::{broyden_root, lu_solve, RootFindOptions};
use crate::columns::model::{
    ColumnError, ColumnSolverInput, ColumnSolverOutput, ColumnSpec, ColumnType, CondenserType,
    SpecType,
};
use crate::columns::profile::StageProfile;
use crate::columns::specs::{evaluate_condenser_spec, evaluate_reboiler_spec, scaled_spec_value};
use crate::columns::thermo_bridge::ColumnThermo;
use crate::columns::tridiagonal::tdma_solve;

/// How the inner loop updates the stage temperatures.
///
/// Ports upstream's integer `Mode` argument to `Solve_Internal`
/// (`BubblePoint.vb:776`). Enum, not an integer, per the workspace design
/// rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureUpdate {
    /// `Mode = 0`: a bubble-point calculation per stage on the current liquid
    /// composition (`BubblePoint.vb:1251-1326`). The Wang-Henke method proper.
    #[default]
    BubblePointFlash,
    /// `Mode = 1`: a Broyden update driving the equilibrium summation residual
    /// `1 − Σ_i K_{i,j} x_{i,j}` to zero on every stage simultaneously
    /// (`BubblePoint.vb:1328-1417`). Upstream switches to this automatically
    /// when a stage's K-value spread exceeds 10 000, i.e. for a wide-boiling
    /// mixture where the bubble-point calculation is ill-conditioned.
    BroydenOnSummation,
}

/// The Wang-Henke bubble-point solver — upstream's `WangHenkeMethod`.
///
/// Stateless apart from the sub-cooling offset, which upstream also keeps as a
/// solver field (`_subcoolingdeltat`, `BubblePoint.vb:37`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WangHenkeSolver {
    /// Condenser sub-cooling \[K\] below the bubble point (0 for a saturated
    /// condenser). Applied as `T_0 -= ΔT_sub` after the bubble-point update
    /// (`BubblePoint.vb:1295`).
    pub subcooling_delta_t: f64,
    /// Temperature-update mode to start in. Upstream always starts at
    /// [`TemperatureUpdate::BubblePointFlash`] and may switch itself.
    pub temperature_update: TemperatureUpdate,
}

impl WangHenkeSolver {
    /// The solver's display name — upstream's `Name` property
    /// (`BubblePoint.vb:39-43`).
    #[must_use]
    pub fn name() -> &'static str {
        "Wang-Henke Solver"
    }

    /// The solver's description — upstream's `Description` property
    /// (`BubblePoint.vb:45-49`).
    #[must_use]
    pub fn description() -> &'static str {
        "Wang-Henke Bubble-Point (BP) Solver"
    }

    /// Solve the column — the entry point equivalent to upstream's
    /// `SolveColumn(input)` (`BubblePoint.vb:1859-2020`).
    ///
    /// Validates the input shape, builds the thermo bridge, runs
    /// [`Self::solve`], and packages the profile into a
    /// [`ColumnSolverOutput`] with both specifications' achieved values filled
    /// in.
    ///
    /// # Errors
    ///
    /// Any [`ColumnError`] from validation, the inner loop, or the outer
    /// root-find.
    pub fn solve_column(
        &self,
        input: &ColumnSolverInput,
    ) -> Result<ColumnSolverOutput, ColumnError> {
        input.validate_shape()?;
        let thermo = ColumnThermo::new(input.components.clone(), input.package);
        let solver = Self {
            subcooling_delta_t: input.subcooling_delta_t,
            ..*self
        };
        let profile = solver.solve(input, &thermo)?;

        let mut cspec = input.condenser_spec.clone();
        let mut rspec = input.reboiler_spec.clone();
        cspec.calculated_value = evaluate_condenser_spec(
            &cspec,
            &profile,
            &thermo,
            &input.feed_flows,
            &input.overall_compositions,
            input.condenser_type,
        )?
        .calculated;
        rspec.calculated_value = evaluate_reboiler_spec(
            &rspec,
            &profile,
            &thermo,
            &input.feed_flows,
            &input.overall_compositions,
        )?
        .calculated;

        Ok(profile.into_output(cspec, rspec))
    }

    /// The outer specification loop — upstream's `Solve`
    /// (`BubblePoint.vb:51-761`).
    ///
    /// Classifies the two user specifications
    /// ([`ColumnSpec::directly_imposable_at_condenser`] /
    /// [`ColumnSpec::directly_imposable_at_reboiler`], ports lines 103-127) and
    /// takes one of four paths:
    ///
    /// 1. **Both directly imposable** — one call to [`Self::solve_internal`]
    ///    (lines 744-757).
    /// 2. **Condenser spec needs the outer loop** — root-find on the reflux
    ///    ratio, substituting a [`SpecType::StreamRatio`] spec into the inner
    ///    loop (lines 334-517).
    /// 3. **Reboiler spec needs the outer loop** — root-find on the bottoms
    ///    molar rate, substituting a [`SpecType::ProductMolarFlowRate`] spec
    ///    (lines 519-742).
    /// 4. **Both need the outer loop** — a 2-D root-find on
    ///    `(reflux ratio, bottoms rate)` (lines 134-332).
    ///
    /// A reboiled absorber has no condenser spec and a refluxed absorber no
    /// reboiler spec, so those are forced "directly imposable" (lines 126-127).
    ///
    /// # Errors
    ///
    /// [`ColumnError::NotConverged`] if the outer root-find cannot meet the
    /// tolerance, or any error the inner loop raises at the best point found.
    pub fn solve(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
    ) -> Result<StageProfile, ColumnError> {
        let cspec = &input.condenser_spec;
        let rspec = &input.reboiler_spec;

        let spec_c_direct = cspec.directly_imposable_at_condenser()
            || input.column_type == ColumnType::ReboiledAbsorber;
        let spec_r_direct = rspec.directly_imposable_at_reboiler()
            || input.column_type == ColumnType::RefluxedAbsorber;

        match (spec_c_direct, spec_r_direct) {
            (true, true) => self.solve_internal(
                input,
                thermo,
                cspec,
                rspec,
                self.temperature_update,
                input.early_stop_iteration,
            ),
            (false, true) => self.solve_outer_condenser(input, thermo),
            (true, false) => self.solve_outer_reboiler(input, thermo),
            (false, false) => self.solve_outer_both(input, thermo),
        }
    }

    /// Initial reflux-ratio guess for the outer loop.
    ///
    /// Ports `BubblePoint.vb:136-145`: the user's `InitialEstimate` if present,
    /// else `(L_0 + LSS_0) / LSS_0` for a condenser with a liquid distillate, or
    /// `(V_1 + F_0)/V_0 − 1` for a full-reflux column.
    fn initial_reflux_ratio(&self, input: &ColumnSolverInput) -> f64 {
        if let Some(v) = input.condenser_spec.initial_estimate {
            return v;
        }
        let l0 = input.liquid_flows[0];
        let lss0 = input.liquid_side_draws[0];
        let v0 = input.vapor_flows[0];
        let v1 = input.vapor_flows.get(1).copied().unwrap_or(0.0);
        let f0 = input.feed_flows[0];
        let rr = if input.condenser_type != CondenserType::FullReflux {
            if lss0 != 0.0 {
                (l0 + lss0) / lss0
            } else {
                2.0
            }
        } else if v0 != 0.0 {
            (v1 + f0) / v0 - 1.0
        } else {
            2.0
        };
        if rr.is_finite() && rr > 0.0 {
            rr
        } else {
            2.0
        }
    }

    /// Outer loop when only the condenser spec needs root-finding
    /// (`BubblePoint.vb:334-517`).
    fn solve_outer_condenser(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
    ) -> Result<StageProfile, ColumnError> {
        let rr0 = self.initial_reflux_ratio(input);
        let mut best: Option<(f64, StageProfile)> = None;
        let tol = input.inner_tolerance();

        let mut residual = |xv: &[f64]| -> Option<Vec<f64>> {
            let mut cspec = ColumnSpec::reflux_ratio(xv[0].abs());
            cspec.stage_number = 0;
            let profile = self
                .solve_internal(
                    input,
                    thermo,
                    &cspec,
                    &input.reboiler_spec,
                    self.temperature_update,
                    input.early_stop_iteration,
                )
                .ok()?;
            let ev = evaluate_condenser_spec(
                &input.condenser_spec,
                &profile,
                thermo,
                &input.feed_flows,
                &input.overall_compositions,
                input.condenser_type,
            )
            .ok()?;
            let e = ev.error?;
            if best.as_ref().is_none_or(|(b, _)| e * e < *b) {
                best = Some((e * e, profile));
            }
            Some(vec![e])
        };

        let r = broyden_root(
            &mut residual,
            &[rr0],
            RootFindOptions {
                tolerance: tol * tol,
                max_iterations: input.max_iterations,
                max_relative_step: 0.5,
                ..RootFindOptions::default()
            },
        );

        match best {
            Some((obj, profile)) if obj.sqrt() <= tol.max(1e-6) => Ok(profile),
            Some((obj, profile)) => {
                let _ = profile;
                Err(ColumnError::NotConverged {
                    iterations: r.iterations,
                    error: obj.sqrt(),
                })
            }
            None => Err(ColumnError::NotConverged {
                iterations: r.iterations,
                error: f64::INFINITY,
            }),
        }
    }

    /// Outer loop when only the reboiler spec needs root-finding
    /// (`BubblePoint.vb:519-742`).
    ///
    /// The independent variable is the bottoms molar rate, clamped to the total
    /// feed rate (`xvars(0) > F.Sum -> 0.99 * F.Sum`, line 691).
    fn solve_outer_reboiler(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
    ) -> Result<StageProfile, ColumnError> {
        let sum_f: f64 = input.feed_flows.iter().sum();
        let b0 = *input.liquid_flows.last().expect("validated non-empty");
        let mut best: Option<(f64, StageProfile)> = None;
        let tol = input.inner_tolerance();

        let mut residual = |xv: &[f64]| -> Option<Vec<f64>> {
            let mut b = xv[0].abs();
            if b > sum_f {
                b = 0.99 * sum_f;
            }
            let rspec = ColumnSpec {
                spec_type: SpecType::ProductMolarFlowRate,
                value: b,
                ..ColumnSpec::default()
            };
            let profile = self
                .solve_internal(
                    input,
                    thermo,
                    &input.condenser_spec,
                    &rspec,
                    self.temperature_update,
                    input.early_stop_iteration,
                )
                .ok()?;
            let ev = evaluate_reboiler_spec(
                &input.reboiler_spec,
                &profile,
                thermo,
                &input.feed_flows,
                &input.overall_compositions,
            )
            .ok()?;
            let e = ev.error?;
            if best.as_ref().is_none_or(|(bb, _)| e * e < *bb) {
                best = Some((e * e, profile));
            }
            Some(vec![e])
        };

        let r = broyden_root(
            &mut residual,
            &[b0],
            RootFindOptions {
                tolerance: tol * tol,
                max_iterations: input.max_iterations,
                max_relative_step: 0.5,
                ..RootFindOptions::default()
            },
        );

        match best {
            Some((obj, profile)) if obj.sqrt() <= tol.max(1e-6) => Ok(profile),
            Some((obj, _)) => Err(ColumnError::NotConverged {
                iterations: r.iterations,
                error: obj.sqrt(),
            }),
            None => Err(ColumnError::NotConverged {
                iterations: r.iterations,
                error: f64::INFINITY,
            }),
        }
    }

    /// Outer loop when **both** specs need root-finding
    /// (`BubblePoint.vb:134-332`) — a 2-D Broyden solve on
    /// `(reflux ratio, bottoms rate)`.
    fn solve_outer_both(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
    ) -> Result<StageProfile, ColumnError> {
        let rr0 = self.initial_reflux_ratio(input);
        let b0 = *input.liquid_flows.last().expect("validated non-empty");
        let sum_f: f64 = input.feed_flows.iter().sum();
        let tol = input.inner_tolerance();
        let mut best: Option<(f64, StageProfile)> = None;

        let mut residual = |xv: &[f64]| -> Option<Vec<f64>> {
            let cspec = ColumnSpec::reflux_ratio(xv[0].abs());
            let mut b = xv[1].abs();
            if b > sum_f {
                b = 0.99 * sum_f;
            }
            let rspec = ColumnSpec {
                spec_type: SpecType::ProductMolarFlowRate,
                value: b,
                ..ColumnSpec::default()
            };
            let profile = self
                .solve_internal(
                    input,
                    thermo,
                    &cspec,
                    &rspec,
                    self.temperature_update,
                    input.early_stop_iteration,
                )
                .ok()?;
            let e1 = evaluate_condenser_spec(
                &input.condenser_spec,
                &profile,
                thermo,
                &input.feed_flows,
                &input.overall_compositions,
                input.condenser_type,
            )
            .ok()?
            .error?;
            let e2 = evaluate_reboiler_spec(
                &input.reboiler_spec,
                &profile,
                thermo,
                &input.feed_flows,
                &input.overall_compositions,
            )
            .ok()?
            .error?;
            let obj = e1 * e1 + e2 * e2;
            if best.as_ref().is_none_or(|(b2, _)| obj < *b2) {
                best = Some((obj, profile));
            }
            Some(vec![e1, e2])
        };

        let r = broyden_root(
            &mut residual,
            &[rr0, b0],
            RootFindOptions {
                tolerance: tol * tol,
                max_iterations: input.max_iterations,
                max_relative_step: 0.5,
                ..RootFindOptions::default()
            },
        );

        match best {
            Some((obj, profile)) if obj.sqrt() <= tol.max(1e-6) => Ok(profile),
            Some((obj, _)) => Err(ColumnError::NotConverged {
                iterations: r.iterations,
                error: obj.sqrt(),
            }),
            None => Err(ColumnError::NotConverged {
                iterations: r.iterations,
                error: f64::INFINITY,
            }),
        }
    }

    /// The Wang-Henke inner loop — upstream's `Solve_Internal`
    /// (`BubblePoint.vb:763-1857`).
    ///
    /// `cspec` and `rspec` must both be directly imposable (see
    /// [`Self::solve`]); the outer loop substitutes surrogate reflux-ratio /
    /// bottoms-rate specs when the user's are not.
    ///
    /// # Parameters
    ///
    /// - `input` — the column definition and starting profile.
    /// - `thermo` — the property-package bridge.
    /// - `cspec` / `rspec` — the condenser-end and reboiler-end specs actually
    ///   imposed on this pass.
    /// - `mode` — starting temperature-update mode; may switch itself to
    ///   [`TemperatureUpdate::BroydenOnSummation`] on a wide-boiling mixture.
    /// - `stop_at` — if `Some(n)`, exit after `n - 1` inner iterations
    ///   regardless of convergence (upstream's `stopatitnumber`, used to run a
    ///   short warm-up for the Newton solver).
    ///
    /// # Errors
    ///
    /// - [`ColumnError::NotConverged`] on exhausting `max_iterations`
    ///   (upstream `DCMaxIterationsReached`, line 1701).
    /// - [`ColumnError::InvalidProfile`] if a temperature/flow profile goes
    ///   non-finite or a composition fails to normalise (lines 1705, 1791-1820).
    /// - [`ColumnError::BubblePointFailed`] from a stage bubble-point
    ///   calculation (line 1283).
    /// - [`ColumnError::SingularMatrix`] from the tridiagonal solve.
    #[allow(clippy::too_many_lines)]
    pub fn solve_internal(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
        cspec: &ColumnSpec,
        rspec: &ColumnSpec,
        mode: TemperatureUpdate,
        stop_at: Option<usize>,
    ) -> Result<StageProfile, ColumnError> {
        let nc = input.n_components();
        let n = input.number_of_stages;
        let ns = input.top_index();
        let tolerance = input.inner_tolerance();
        let maxits = input.max_iterations;
        let mut mode = mode;

        let rebabs = input.column_type == ColumnType::ReboiledAbsorber;
        let refabs = input.column_type == ColumnType::RefluxedAbsorber;
        let condt = input.condenser_type;

        // step0 — spec values in SI, percent scaling applied (lines 876-901).
        let spval1 = scaled_spec_value(cspec);
        let spval2 = scaled_spec_value(rspec);

        // step2 — working copies (lines 911-934).
        let p = &input.stage_pressures;
        let f = &input.feed_flows;
        let fcj = &input.feed_compositions;
        let eff = &input.stage_efficiencies;
        let hfj = &input.feed_enthalpies; // [J/mol], no /1000 (see model.rs)

        let mut tj = input.stage_temperatures.clone();
        let mut vj = input.vapor_flows.clone();
        let mut lj = input.liquid_flows.clone();
        let vssj = input.vapor_side_draws.clone();
        let mut lssj = input.liquid_side_draws.clone();
        let mut k = input.k_values.clone();
        let mut k_ant = k.clone();
        let mut q = input.stage_heats.clone();
        let x_frozen = input.liquid_compositions.clone(); // upstream's never-written `x`
        let mut xc = input.liquid_compositions.clone();
        let mut yc = input.vapor_compositions.clone();

        // Initial phase enthalpies (lines 961-967).
        let mut hl: Vec<f64> = (0..n)
            .map(|i| thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]))
            .collect();
        let mut hv: Vec<f64> = (0..n)
            .map(|i| thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]))
            .collect();

        let mut sum_f: f64 = f.iter().sum();
        let mut sum_lss: f64 = input.liquid_side_draws.iter().skip(1).sum();
        let mut sum_vss: f64 = vssj.iter().sum();

        // Condenser spec -> LSS_0 and reflux ratio (lines 974-995). The
        // pre-loop call is made for its side effects on `LSS_0` and `Q_0`; the
        // reflux ratio itself is recomputed at the top of every iteration.
        let mut rr: f64;
        let _pre_loop_rr = impose_condenser_spec(
            cspec, spval1, rebabs, thermo, &xc, &mut lssj, &mut q, &lj, &vj, &vssj, f, hfj, &hl,
            &hv, sum_f,
        );

        // Reboiler spec -> bottoms rate B (lines 997-1021).
        let mut b;
        b = impose_reboiler_spec(
            rspec, spval2, refabs, thermo, &xc, &lssj, &vssj, &lj, &vj, f, hfj, &hl, &hv, &mut q,
            sum_f, ns,
        );

        // Condenser mass balance (lines 1023-1035).
        if rebabs {
            vj[0] = sum_f - b - sum_lss - sum_vss;
            lssj[0] = 0.0;
            lj[ns] = b;
        } else if condt == CondenserType::FullReflux {
            vj[0] = sum_f - b - sum_lss - sum_vss;
            lssj[0] = 0.0;
        } else {
            lssj[0] = sum_f - b - sum_lss - sum_vss - vj[0];
        }

        // step3 (lines 1088-1097).
        let mut max_dt = 50.0_f64;
        let mut kfac = vec![1.0_f64; n];
        let mut broyden_t = BroydenTemperature::new(n);
        let mut t_error: f64;
        let mut vf_error: f64;
        let mut ic: usize = 0;

        loop {
            // step4 — tridiagonal coefficients (lines 1131-1164).
            for i in 0..n {
                for j in 0..nc {
                    if !k[i][j].is_finite() || k[i][j] <= 0.0 {
                        k[i][j] = thermo.ideal_k_value(j, tj[i], p[i]);
                    }
                }
            }

            let (sum1, sum2) = net_flow_sums(f, &lssj, &vssj, n);

            let mut lc = vec![vec![0.0_f64; nc]; n];
            for j in 0..nc {
                let mut at = vec![0.0_f64; n];
                let mut bt = vec![0.0_f64; n];
                let mut ct = vec![0.0_f64; n];
                let mut dt = vec![0.0_f64; n];
                for i in 0..n {
                    dt[i] = -f[i] * fcj[i][j];
                    bt[i] = if i < ns {
                        -(vj[i + 1] + sum1[i] - vj[0] + lssj[i] + (vj[i] + vssj[i]) * k[i][j])
                    } else {
                        -(sum1[i] - vj[0] + lssj[i] + (vj[i] + vssj[i]) * k[i][j])
                    };
                    if i < ns {
                        ct[i] = vj[i + 1] * k[i + 1][j];
                    }
                    if i > 0 {
                        at[i] = vj[i] + sum2[i] - vj[0];
                    }
                }
                let xt = tdma_solve(&at, &bt, &ct, &dt)?;
                for i in 0..n {
                    // Upstream takes the absolute value of a negative component
                    // liquid flow (lines 1209-1211) rather than clamping to zero.
                    lc[i][j] = xt[i].abs();
                }
            }

            // Normalise to stage liquid compositions (lines 1216-1233).
            if ic > 0 {
                for i in 0..n {
                    let sumx: f64 = lc[i].iter().sum();
                    for j in 0..nc {
                        xc[i][j] = if sumx > 0.0 {
                            lc[i][j] / sumx
                        } else if k[i][j] > 0.0 {
                            yc[i][j] / k[i][j]
                        } else {
                            0.0
                        };
                    }
                }
            } else {
                xc.clone_from(&input.liquid_compositions);
            }

            // step5 — new stage temperatures.
            let tj_ant = tj.clone();
            if mode == TemperatureUpdate::BubblePointFlash || ic == 0 {
                for i in 0..n {
                    match thermo.bubble_temperature(&xc[i], p[i], tj[i], i) {
                        Ok((t_new, k_new)) => {
                            k_ant[i].clone_from(&k[i]);
                            if t_new > 0.0 && t_new.is_finite() {
                                tj[i] = t_new;
                                k[i] = k_new;
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                tj[0] -= self.subcooling_delta_t;

                // Wide-boiling detection (lines 1301-1308).
                for i in 0..n {
                    kfac[i] = k_spread(&k[i], &xc[i]);
                }
                if kfac.iter().cloned().fold(0.0_f64, f64::max) > 10_000.0 {
                    mode = TemperatureUpdate::BroydenOnSummation;
                }

                let d_tj: Vec<f64> = (0..n).map(|i| tj[i] - tj_ant[i]).collect();
                let max_abs_dt = d_tj.iter().fold(0.0_f64, |a, d| a.max(d.abs()));
                let af = if max_abs_dt > max_dt && max_abs_dt > 0.0 {
                    max_dt / max_abs_dt
                } else {
                    1.0
                };
                // See "Faithfully ported upstream quirks": `max_dt` starts at
                // exactly 50, so this branch is only taken after a Mode-1 pass.
                if max_dt < 50.0 {
                    for i in 0..n {
                        let t_new = tj_ant[i] + af * d_tj[i];
                        if t_new > 0.0 && t_new.is_finite() {
                            tj[i] = t_new;
                        } else {
                            tj[i] = tj_ant[i];
                            k[i].clone_from(&k_ant[i]);
                        }
                    }
                } else {
                    for i in 0..n {
                        tj[i] = tj[i] / 2.0 + tj_ant[i] / 2.0;
                    }
                }
            } else {
                // Mode 1 — Broyden on the equilibrium summation residual.
                // NOTE: upstream evaluates this at the *frozen* input `x`
                // (line 1331), not at `xc`. Preserved; see the module header.
                let fx: Vec<f64> = (0..n)
                    .map(|i| 1.0 - (0..nc).map(|j| k[i][j] * x_frozen[i][j]).sum::<f64>())
                    .collect();
                let dxtj = broyden_t.step(&tj, &fx, ic < 3);

                max_dt = if kfac.iter().cloned().fold(0.0_f64, f64::max) > 10_000.0 {
                    5.0
                } else {
                    50.0
                };
                let max_abs = dxtj.iter().fold(0.0_f64, |a, d| a.max(d.abs()));
                let af = if max_abs > max_dt && max_abs > 0.0 {
                    max_dt / max_abs
                } else {
                    1.0
                };
                for i in 0..n {
                    let t_new = tj[i] + af * dxtj[i];
                    if t_new > 0.0 && t_new.is_finite() {
                        tj[i] = t_new;
                    } else {
                        tj[i] = tj_ant[i];
                        k[i].clone_from(&k_ant[i]);
                    }
                }
                for i in 0..n {
                    k_ant[i].clone_from(&k[i]);
                    let t_eff = if i == 0 {
                        tj[i] - self.subcooling_delta_t
                    } else {
                        tj[i]
                    };
                    k[i] = thermo.k_values(&xc[i], &yc[i], t_eff, p[i]);
                }
            }

            // NaN guards (lines 1420-1429).
            for i in 0..n {
                if !tj[i].is_finite() {
                    tj[i] = tj_ant[i];
                }
                for j in 0..nc {
                    if !k[i][j].is_finite() || k[i][j] <= 0.0 {
                        k[i][j] = thermo.ideal_k_value(j, tj[i], p[i]);
                    }
                }
            }

            t_error = (0..n).map(|i| (tj[i] - tj_ant[i]).powi(2)).sum();

            // step6 — vapour compositions with stage efficiency (lines 1441-1457).
            for i in (0..n).rev() {
                let mut sumy = 0.0;
                for j in 0..nc {
                    yc[i][j] = if i == ns {
                        k[i][j] * xc[i][j]
                    } else {
                        eff[i] * k[i][j] * xc[i][j] + (1.0 - eff[i]) * yc[i + 1][j]
                    };
                    sumy += yc[i][j];
                }
                if sumy > 0.0 {
                    for j in 0..nc {
                        yc[i][j] /= sumy;
                    }
                }
            }

            // Refresh enthalpies at the new profile (lines 1477-1483).
            for i in 0..n {
                hl[i] = thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]);
                hv[i] = thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]);
            }

            // Re-impose the specs on the updated profile (lines 1488-1537).
            rr = impose_condenser_spec(
                cspec, spval1, rebabs, thermo, &xc, &mut lssj, &mut q, &lj, &vj, &vssj, f, hfj,
                &hl, &hv, sum_f,
            );
            b = impose_reboiler_spec(
                rspec, spval2, refabs, thermo, &xc, &lssj, &vssj, &lj, &vj, f, hfj, &hl, &hv,
                &mut q, sum_f, ns,
            );

            sum_f = f.iter().sum();
            sum_lss = input.liquid_side_draws.iter().skip(1).sum();
            sum_vss = vssj.iter().sum();

            if condt == CondenserType::FullReflux || rebabs {
                vj[0] = (sum_f - b - sum_lss - sum_vss).abs();
                lssj[0] = 0.0;
            } else {
                lssj[0] = sum_f - b - sum_lss - sum_vss - vj[0];
            }

            // step7 — energy balance, solved forward for V (lines 1558-1603).
            let (sum1, sum2) = net_flow_sums(f, &lssj, &vssj, n);
            let mut alpha = vec![0.0_f64; n];
            let mut beta = vec![0.0_f64; n];
            let mut gamma = vec![0.0_f64; n];
            for j in 1..n {
                gamma[j] = (sum2[j] - vj[0]) * (hl[j] - hl[j - 1])
                    + f[j] * (hl[j] - hfj[j])
                    + vssj[j] * (hv[j] - hl[j])
                    + q[j];
                alpha[j] = hl[j - 1] - hv[j];
                if j < ns {
                    beta[j] = hv[j + 1] - hl[j];
                }
            }

            let vj_ant = vj.clone();
            if rebabs {
                vj[1] = lj[0] - f[0] + lssj[0] + vssj[0] + vj[0];
            } else if condt != CondenserType::FullReflux {
                vj[0] = input.vapor_flows[0];
                vj[1] = (rr + 1.0) * lssj[0] - f[0] + vj[0];
            } else {
                vj[1] = (rr + 1.0) * vj[0] - f[0];
            }
            for i in 2..n {
                let denom = beta[i - 1];
                vj[i] = if denom != 0.0 && denom.is_finite() {
                    (gamma[i - 1] - alpha[i - 1] * vj[i - 1]) / denom
                } else {
                    vj_ant[i]
                };
                if !vj[i].is_finite() {
                    vj[i] = vj_ant[i];
                }
                if vj[i] < 0.0 {
                    vj[i] = 1.0e-10;
                }
            }
            for i in 1..ns {
                vj[i] = eff[i] * vj[i] + (1.0 - eff[i]) * vj[i + 1];
            }

            vf_error = (0..n)
                .map(|i| {
                    let d = (vj[i] - vj_ant[i]) / (vj_ant[i] + 1.0e-10);
                    d * d
                })
                .sum();

            // Liquid flows from the total mass balance (lines 1609-1628).
            for i in 0..n {
                if i < ns {
                    if i == 0 {
                        if rebabs {
                            lj[0] = if hl[0] != 0.0 {
                                (vj[1] * hv[1] + f[0] * hfj[0]
                                    - (vj[0] + vssj[0]) * hv[0]
                                    - lssj[0] * hl[0])
                                    / hl[0]
                            } else {
                                lj[0]
                            };
                        } else if lssj[0] > 0.0 {
                            lj[0] = rr * lssj[0];
                        } else {
                            lj[0] = vj[1] + sum1[0] - vj[0];
                        }
                    } else {
                        lj[i] = vj[i + 1] + sum1[i] - vj[0];
                    }
                } else {
                    lj[i] = sum1[i] - vj[0];
                }
                if !lj[i].is_finite() || lj[i] < 0.0 {
                    lj[i] = 1.0e-4 * sum_f;
                }
            }

            // End-stage duties from the energy balance (lines 1645-1673).
            back_calculate_duties(
                input.column_type,
                cspec,
                rspec,
                &mut q,
                &lj,
                &vj,
                &lssj,
                &vssj,
                f,
                hfj,
                &hl,
                &hv,
                ns,
                rebabs,
            );

            ic += 1;

            if ic >= maxits {
                return Err(ColumnError::NotConverged {
                    iterations: ic,
                    error: t_error + vf_error,
                });
            }
            for i in 0..n {
                if !tj[i].is_finite() || !vj[i].is_finite() || !lj[i].is_finite() {
                    return Err(ColumnError::InvalidProfile {
                        stage: i,
                        detail: "non-finite temperature or flow".into(),
                    });
                }
            }
            if let Some(stop) = stop_at {
                if ic + 1 >= stop {
                    break;
                }
            }
            if (t_error + vf_error) < tolerance * (ns as f64) / 100.0 && ic > 1 {
                break;
            }
        }

        // Mass-balance sanity check (lines 1791-1820).
        for i in 0..n {
            let sy: f64 = yc[i].iter().sum();
            let sx: f64 = xc[i].iter().sum();
            if (sy - 1.0).abs() > 1.0e-3 || (sx - 1.0).abs() > 1.0e-3 {
                return Err(ColumnError::InvalidProfile {
                    stage: i,
                    detail: format!("compositions do not normalise (Σy = {sy}, Σx = {sx})"),
                });
            }
            if lj[i] < 0.0 || vj[i] < 0.0 || lssj[i] < 0.0 {
                return Err(ColumnError::InvalidProfile {
                    stage: i,
                    detail: format!(
                        "negative flow (L = {}, V = {}, LSS = {})",
                        lj[i], vj[i], lssj[i]
                    ),
                });
            }
        }

        // A converged residual is not on its own evidence of a physical
        // answer. Both of the modelling errors recorded in bead op-190j.5
        // converged to a small residual (2.0e-7, 6.7e-7) on a temperature
        // profile that dipped mid-column -- and a column cannot get colder
        // going down. Without this check a wrong answer is indistinguishable
        // from a right one by return value, which is how both got as far as
        // they did.
        //
        // Stages carrying a specified heat duty are exempt: an intercooler or
        // pumparound legitimately reverses the gradient across itself, so an
        // inversion there is a design, not a defect.
        check_monotonic_temperature(&tj, &input.stage_heats)?;

        Ok(StageProfile {
            temperatures: tj,
            vapor_flows: vj,
            liquid_flows: lj,
            vapor_side_draws: vssj,
            liquid_side_draws: lssj,
            vapor_compositions: yc,
            liquid_compositions: xc,
            k_values: k,
            heats: q,
            iterations: ic,
            error: t_error + vf_error,
        })
    }
}

/// Reject a converged profile whose temperature falls going down the column.
///
/// Stage 0 is the top. Gravity of the check: a distillation column's
/// temperature rises monotonically from condenser to reboiler because each
/// stage below is richer in the heavier components. A dip means the solve has
/// settled somewhere unphysical however small its residual.
///
/// `heats` is the *specified* per-stage duty; a non-zero entry marks a stage
/// with deliberate heating or cooling, across which a reversal is expected and
/// is not flagged. The tolerance absorbs numerical wobble on a flat profile;
/// it is deliberately loose enough not to fire on a well-converged column and
/// tight enough to catch the multi-kelvin dips that motivated it.
pub(crate) fn check_monotonic_temperature(t: &[f64], heats: &[f64]) -> Result<(), ColumnError> {
    /// Kelvin a stage may sit below the one above before it counts as a dip.
    const TOLERANCE_K: f64 = 0.5;
    for i in 1..t.len() {
        let duty_above = heats.get(i - 1).copied().unwrap_or(0.0);
        let duty_here = heats.get(i).copied().unwrap_or(0.0);
        if duty_above != 0.0 || duty_here != 0.0 {
            continue;
        }
        if t[i] < t[i - 1] - TOLERANCE_K {
            return Err(ColumnError::InvalidProfile {
                stage: i,
                detail: format!(
                    "temperature falls going down the column: stage {} is {:.2} K, \
                     stage {i} is {:.2} K ({:.2} K colder). A converged residual \
                     on a non-monotonic profile is a wrong answer, not a solution.",
                    i - 1,
                    t[i - 1],
                    t[i],
                    t[i - 1] - t[i]
                ),
            });
        }
    }
    Ok(())
}

/// `sum1_j = Σ_{m<=j} (F_m − U_m − W_m)` and `sum2_j = Σ_{m<j} (…)`.
///
/// Ports `BubblePoint.vb:1139-1150` (and the identical block at `:1560-1571`).
/// `U` is the liquid side draw, `W` the vapour side draw. Units \[mol/s\].
pub(crate) fn net_flow_sums(f: &[f64], lss: &[f64], vss: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut sum1 = vec![0.0_f64; n];
    let mut sum2 = vec![0.0_f64; n];
    let mut running = 0.0_f64;
    for i in 0..n {
        sum2[i] = running;
        running += f[i] - lss[i] - vss[i];
        sum1[i] = running;
    }
    (sum1, sum2)
}

/// `K_max / K_min` over the components actually present on a stage.
///
/// Ports `K(i).MaxY_NonZero(xc(i)) / K(i).MinY_NonZero(xc(i))`
/// (`BubblePoint.vb:1302`) — the wide-boiling indicator. Components with zero
/// mole fraction are skipped so a trace species does not trip the test.
pub(crate) fn k_spread(k: &[f64], x: &[f64]) -> f64 {
    let mut lo = f64::INFINITY;
    let mut hi = 0.0_f64;
    for (i, &ki) in k.iter().enumerate() {
        if x.get(i).copied().unwrap_or(0.0) > 0.0 && ki > 0.0 && ki.is_finite() {
            lo = lo.min(ki);
            hi = hi.max(ki);
        }
    }
    if lo.is_finite() && lo > 0.0 {
        hi / lo
    } else {
        1.0
    }
}

/// Impose the condenser-end specification, returning the reflux ratio.
///
/// Ports the `specs("C")` `Select Case` of `BubblePoint.vb:974-995`
/// (repeated verbatim at `:1488-1509`). Writes `LSS_0` and, for a heat-duty
/// spec, `Q_0`.
///
/// A reboiled absorber has no condenser: `LSS_0 = 0` and the "reflux ratio" is
/// the stripping ratio `(V_1 + F_0)/V_0 − 1` (lines 992-995).
#[allow(clippy::too_many_arguments)]
pub(crate) fn impose_condenser_spec(
    cspec: &ColumnSpec,
    spval1: f64,
    rebabs: bool,
    thermo: &ColumnThermo,
    xc: &[Vec<f64>],
    lssj: &mut [f64],
    q: &mut [f64],
    lj: &[f64],
    vj: &[f64],
    vssj: &[f64],
    f: &[f64],
    hfj: &[f64],
    hl: &[f64],
    hv: &[f64],
    sum_f: f64,
) -> f64 {
    if rebabs {
        lssj[0] = 0.0;
        let v1 = vj.get(1).copied().unwrap_or(0.0);
        return if vj[0] != 0.0 {
            (v1 + f[0]) / vj[0] - 1.0
        } else {
            0.0
        };
    }
    match cspec.spec_type {
        SpecType::FeedRecovery => lssj[0] = spval1 * sum_f,
        SpecType::ProductMassFlowRate => {
            let mm = thermo.mixture_molar_mass(&xc[0]);
            if mm > 0.0 {
                lssj[0] = spval1 / mm;
            }
        }
        SpecType::ProductMolarFlowRate => lssj[0] = spval1,
        SpecType::StreamRatio => return spval1,
        SpecType::HeatDuty => {
            q[0] = spval1;
            let v1 = vj.get(1).copied().unwrap_or(0.0);
            let hv1 = hv.get(1).copied().unwrap_or(0.0);
            if hl[0] != 0.0 {
                lssj[0] =
                    -lj[0] - (q[0] - v1 * hv1 - f[0] * hfj[0] + (vj[0] + vssj[0]) * hv[0]) / hl[0];
            }
        }
        _ => {}
    }
    if lssj[0] != 0.0 {
        lj[0] / lssj[0]
    } else {
        0.0
    }
}

/// Impose the reboiler-end specification, returning the bottoms molar rate `B`
/// \[mol/s\].
///
/// Ports the `specs("R")` `Select Case` of `BubblePoint.vb:997-1021`
/// (repeated at `:1511-1537`). Writes `Q_ns` for a heat-duty spec. A refluxed
/// absorber has no reboiler: `B = L_ns` (line 1020).
#[allow(clippy::too_many_arguments)]
pub(crate) fn impose_reboiler_spec(
    rspec: &ColumnSpec,
    spval2: f64,
    refabs: bool,
    thermo: &ColumnThermo,
    xc: &[Vec<f64>],
    lssj: &[f64],
    vssj: &[f64],
    lj: &[f64],
    vj: &[f64],
    f: &[f64],
    hfj: &[f64],
    hl: &[f64],
    hv: &[f64],
    q: &mut [f64],
    sum_f: f64,
    ns: usize,
) -> f64 {
    if refabs {
        return lj[ns];
    }
    match rspec.spec_type {
        SpecType::FeedRecovery => spval2 * sum_f,
        SpecType::ProductMassFlowRate => {
            let mm = thermo.mixture_molar_mass(&xc[ns]);
            if mm > 0.0 {
                spval2 / mm
            } else {
                lj[ns]
            }
        }
        SpecType::ProductMolarFlowRate => spval2,
        SpecType::HeatDuty => {
            q[ns] = -spval2;
            let sum3: f64 = (0..=ns)
                .map(|i| f[i] * hfj[i] - lssj[i] * hl[i] - vssj[i] * hv[i])
                .sum();
            let sum4: f64 = (0..ns).map(|i| q[i]).sum();
            if hl[ns] != 0.0 {
                (sum3 - sum4 - vj[0] * hv[0] - q[ns]) / hl[ns]
            } else {
                lj[ns]
            }
        }
        _ => lj[ns],
    }
}

/// Back-calculate the condenser and/or reboiler duties from the end-stage
/// energy balances.
///
/// Ports `BubblePoint.vb:1645-1673` — the `Select Case coltype` block. A duty
/// that was *specified* is left alone; an absorption column keeps both user
/// values.
#[allow(clippy::too_many_arguments)]
pub(crate) fn back_calculate_duties(
    coltype: ColumnType,
    cspec: &ColumnSpec,
    rspec: &ColumnSpec,
    q: &mut [f64],
    lj: &[f64],
    vj: &[f64],
    lssj: &[f64],
    vssj: &[f64],
    f: &[f64],
    hfj: &[f64],
    hl: &[f64],
    hv: &[f64],
    ns: usize,
    rebabs: bool,
) {
    let condenser_duty = |q: &[f64]| {
        let _ = q;
        let v1 = vj.get(1).copied().unwrap_or(0.0);
        let hv1 = hv.get(1).copied().unwrap_or(0.0);
        v1 * hv1 + f[0] * hfj[0] - (lj[0] + lssj[0]) * hl[0] - (vj[0] + vssj[0]) * hv[0]
    };
    let reboiler_duty_total = |q: &[f64]| {
        let sum3: f64 = (0..=ns)
            .map(|i| f[i] * hfj[i] - lssj[i] * hl[i] - vssj[i] * hv[i])
            .sum();
        let sum4: f64 = (0..ns).map(|i| q[i]).sum();
        sum3 - sum4 - vj[0] * hv[0] - lj[ns] * hl[ns]
    };

    match coltype {
        ColumnType::DistillationColumn => {
            if cspec.spec_type != SpecType::HeatDuty {
                q[0] = condenser_duty(q);
            }
            if rspec.spec_type != SpecType::HeatDuty {
                q[ns] = reboiler_duty_total(q);
            }
            if rebabs {
                q[0] = 0.0;
            }
        }
        ColumnType::AbsorptionColumn => {
            // Use the provided values (upstream line 1664).
        }
        ColumnType::RefluxedAbsorber => {
            if cspec.spec_type != SpecType::HeatDuty {
                q[0] = condenser_duty(q);
            }
        }
        ColumnType::ReboiledAbsorber => {
            if rspec.spec_type != SpecType::HeatDuty {
                q[ns] = lj[ns - 1] * hl[ns - 1] + f[ns] * hfj[ns]
                    - (lj[ns] + lssj[ns]) * hl[ns]
                    - (vj[ns] + vssj[ns]) * hv[ns];
            }
        }
    }
}

/// The persistent Broyden state behind upstream's
/// `DWSIM.MathOps.MathEx.Optimization.Broyden.broydn`
/// (`BubblePoint.vb:1344`, `:1348`).
///
/// `broydn(n, x, f, dx, xold, fold, jac, mode)` with `mode = 0` initialises
/// from the caller-supplied Jacobian (upstream sets it to the identity for the
/// first three iterations, lines 1338-1342) and `mode = 1` applies the rank-1
/// Broyden update. In both cases it returns `dx = −J⁻¹ f` and remembers
/// `(x, f)`.
#[derive(Debug, Clone)]
struct BroydenTemperature {
    jac: Array2<f64>,
    x_old: Vec<f64>,
    f_old: Vec<f64>,
    have_history: bool,
}

impl BroydenTemperature {
    fn new(n: usize) -> Self {
        Self {
            jac: Array2::<f64>::eye(n),
            x_old: vec![0.0; n],
            f_old: vec![0.0; n],
            have_history: false,
        }
    }

    /// One `broydn` step. `reset_identity` reproduces upstream's `ic < 3`
    /// branch, which rebuilds the Jacobian as the identity before each of the
    /// first three updates.
    fn step(&mut self, x: &[f64], f: &[f64], reset_identity: bool) -> Vec<f64> {
        let n = x.len();
        if reset_identity {
            self.jac = Array2::<f64>::eye(n);
        } else if self.have_history {
            let dx: Vec<f64> = (0..n).map(|i| x[i] - self.x_old[i]).collect();
            let df: Vec<f64> = (0..n).map(|i| f[i] - self.f_old[i]).collect();
            let denom: f64 = dx.iter().map(|d| d * d).sum();
            if denom > 0.0 && denom.is_finite() {
                let mut jdx = vec![0.0_f64; n];
                for r in 0..n {
                    let mut s = 0.0;
                    for c in 0..n {
                        s += self.jac[[r, c]] * dx[c];
                    }
                    jdx[r] = s;
                }
                for r in 0..n {
                    let num = df[r] - jdx[r];
                    if !num.is_finite() {
                        continue;
                    }
                    for c in 0..n {
                        self.jac[[r, c]] += num * dx[c] / denom;
                    }
                }
            }
        }
        self.x_old = x.to_vec();
        self.f_old = f.to_vec();
        self.have_history = true;

        let rhs: Vec<f64> = f.iter().map(|v| -v).collect();
        lu_solve(self.jac.clone(), &rhs).unwrap_or_else(|| vec![0.0; n])
    }
}

#[cfg(test)]
mod monotonicity_tests {
    use super::check_monotonic_temperature;

    #[test]
    fn a_rising_profile_passes() {
        let t = [355.0, 360.0, 366.0, 372.0, 380.0];
        assert!(check_monotonic_temperature(&t, &[0.0; 5]).is_ok());
    }

    /// The shape both modelling errors in bead op-190j.5 produced: converged,
    /// small residual, and a dip in the middle.
    #[test]
    fn a_dip_mid_column_is_rejected() {
        let t = [355.0, 372.0, 361.0, 375.0, 380.0];
        let err = check_monotonic_temperature(&t, &[0.0; 5])
            .expect_err("a column that gets colder going down must not pass");
        let msg = format!("{err}");
        assert!(msg.contains("falls going down"), "unhelpful message: {msg}");
    }

    /// A flat, well-converged profile must not trip on numerical wobble.
    #[test]
    fn sub_tolerance_wobble_is_not_a_dip() {
        let t = [400.0, 399.9, 400.1, 400.0];
        assert!(check_monotonic_temperature(&t, &[0.0; 4]).is_ok());
    }

    /// An intercooler or pumparound is allowed to reverse the gradient across
    /// itself; that is a design, not a defect.
    #[test]
    fn a_stage_with_a_specified_duty_may_reverse() {
        let t = [355.0, 372.0, 350.0, 375.0];
        let heats = [0.0, 0.0, -5.0e5, 0.0];
        assert!(check_monotonic_temperature(&t, &heats).is_ok());
    }
}
