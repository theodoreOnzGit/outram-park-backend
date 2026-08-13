//! Burningham-Otto sum-rates (SR) method for absorbers and strippers.
//!
//! Pure-Rust port of DWSIM's
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumnSolvers/SumRates.vb`
//! (GPL-3.0), class `BurninghamOttoMethod`, upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al.
//!
//! | This module | Upstream member | `SumRates.vb` lines |
//! |---|---|---|
//! | [`SumRatesSolver::solve`] | `Public Shared Function Solve` | 49-808 |
//! | [`SumRatesSolver::solve_column`] | `Public Overrides Function SolveColumn` | 810-895 |
//! | [`damping_factor`] | `MathEx.Interpolation.GetDampingFactor` (`DWSIM.Math/Interpolation.vb:39-48`) | — |
//!
//! # Method provenance
//!
//! Sujata's sum-rates idea as developed by **Burningham, D. W. & Otto, F. D.
//! (1967)**, "Which computer design for absorbers?", *Hydrocarbon Processing*
//! **46**(10), 163-170, in the tridiagonal formulation of Friday & Smith.
//! Upstream's own doc paragraphs (`SumRates.vb:69-137`) state the motivation
//! verbatim: for **wide-boiling** mixtures the Wang-Henke bubble-point method
//! fails, because the bubble-point temperature is far too sensitive to the
//! liquid composition while the stage energy balance is far more sensitive to
//! the stage temperatures than to the interstage flows. Sum-rates inverts the
//! roles:
//!
//! 1. **M** — same tridiagonal solve as Wang-Henke, giving un-normalised
//!    component liquid flows `l_{i,j}` (see [`crate::columns::tridiagonal`]).
//! 2. **Sum rates** — the *total* liquid flows come from summing them:
//!    `L_j^{(k+1)} = L_j^{(k)} Σ_i l_{i,j}`. This is the step the method is
//!    named for. Compositions are then `x_{i,j} = l_{i,j} / Σ_i l_{i,j}` and
//!    `y_{i,j} = K_{i,j} x_{i,j}` (normalised).
//! 3. **Vapour flows** from the total mass balance around the bottom:
//!    `V_j = L_{j-1} − L_N + Σ_{m>=j}(F_m − U_m − W_m)`.
//! 4. **H** — the stage energy balances are solved for temperature *corrections*
//!    by Newton-Raphson, with `∂H_j/∂T` obtained from central differences of
//!    the phase enthalpies. The Jacobian is tridiagonal (stage `j`'s balance
//!    depends on `T_{j-1}`, `T_j`, `T_{j+1}` only), so the same Thomas solver
//!    delivers `ΔT` in one pass.
//!
//! Convergence is on both the temperature change and the vapour-composition
//! change, and is described upstream as "rapid".
//!
//! # Units
//!
//! Documented raw `f64` in SI: `T` \[K\], `P` \[Pa\], flows \[mol/s\], molar
//! enthalpies \[J/mol\], duties \[W\], compositions and K-values \[-\]. The
//! enthalpy temperature-derivative `dH/dT` is \[J/(mol·K)\].
//!
//! # Excluded DWSIM behavior
//!
//! - **`Inspector` paragraphs and the convergence-report `StringBuilder`**
//!   (lines 63-137, 227-272, 710-729, 737-786) — reporting, no numerics.
//! - **`Parallel.For` branches** (lines 455-494). Serial here; identical
//!   arithmetic.
//! - **`llextr`** (the liquid-liquid extractor flavour, lines 210-215, 463-466,
//!   504-509, 620-621, 633-634, 649-651): DWSIM reuses this solver for a
//!   liquid-liquid extractor by treating the "vapour" phase as a second liquid.
//!   Not ported — this port has no extractor mode.
//! - **`IdealK` / `IdealH` warm-up with a substituted `RaoultPropertyPackage`**
//!   (lines 139-145, 459-471, 618-630). The flags survive as
//!   [`crate::columns::model::SolvingScheme`] but no package substitution is
//!   performed; see that enum's docs.
//! - **`DW_CalcEnthalpyOfReaction`** (lines 485-489, 536-540): reactive
//!   distillation, guarded upstream by `pp.HasReactivePhase`. Identically zero
//!   here.
//! - **`pp.CurrentMaterialStream.Flowsheet.CheckStatus()`** cancellation checks.
//!
//! # Faithfully ported upstream quirks (documented, not "fixed")
//!
//! - The **damping factor `af = 5 / max|ΔT|` is computed and never used**
//!   (lines 596-600). The temperature update is the fixed
//!   `T_j += 0.7 ΔT_j` of line 611, with no step cap. Preserved: introducing the
//!   cap would change the iteration path.
//! - `RelaxCompositionUpdates` exists as a flag and its body is **commented out**
//!   upstream (lines 659-661), so composition relaxation never happens. Ported
//!   as [`SumRatesSolver::relax_composition_updates`], which is likewise inert,
//!   and documented as such rather than silently dropped.
//! - The loop cannot exit before **11** iterations (`And ic > 10`, line 788),
//!   even if the tolerances are met at iteration 2.
//! - Stage heat duties `Q_j` are **never updated** by this solver — an absorber
//!   has given duties. They are echoed back unchanged.

use crate::columns::bubble_point::net_flow_sums;
use crate::columns::model::{ColumnError, ColumnSolverInput, ColumnSolverOutput};
use crate::columns::profile::StageProfile;
use crate::columns::specs::{evaluate_condenser_spec, evaluate_reboiler_spec};
use crate::columns::thermo_bridge::ColumnThermo;
use crate::columns::tridiagonal::tdma_solve;

/// Finite-difference step \[K\] for `dH/dT`, upstream's `epsilon`
/// (`SumRates.vb:453`).
const DHDT_EPSILON: f64 = 0.01;

/// Minimum iterations before the convergence test may pass — upstream's
/// `And ic > 10` (`SumRates.vb:788`).
const MIN_ITERATIONS: usize = 10;

/// Ramped damping factor — upstream's
/// `MathEx.Interpolation.GetDampingFactor(current, count, min, max)`
/// (`DWSIM.Math/Interpolation.vb:39-48`).
///
/// `df = (max − min)/count · current`, clamped to `[min, max]`: a linear ramp
/// that starts at `min` and reaches `max` after `count` iterations, so early
/// iterations are heavily damped and later ones take full steps.
///
/// # Parameters
///
/// - `current` — the iteration counter.
/// - `count` — the iteration at which the ramp saturates (upstream passes 50).
/// - `min` / `max` — the clamp bounds \[-\] (upstream passes 0.25 and 1.0).
#[must_use]
pub fn damping_factor(current: usize, count: usize, min: f64, max: f64) -> f64 {
    if count == 0 {
        return max;
    }
    let df = (max - min) / count as f64 * current as f64;
    df.clamp(min, max)
}

/// The Burningham-Otto sum-rates solver.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SumRatesSolver {
    /// Apply the ramped [`damping_factor`] to the temperature update — upstream's
    /// `RelaxTemperatureUpdates` shared property (`SumRates.vb:35`), default
    /// `false`.
    pub relax_temperature_updates: bool,
    /// Upstream's `RelaxCompositionUpdates` shared property
    /// (`SumRates.vb:33`), default `false`. **Inert**: the code it would gate is
    /// commented out upstream (lines 659-661) and is likewise inert here. Kept
    /// so the ported surface matches, and so a reader is not left wondering
    /// where it went.
    pub relax_composition_updates: bool,
}

impl SumRatesSolver {
    /// The solver's display name.
    ///
    /// Upstream's `Name` property throws `NotImplementedException`
    /// (`SumRates.vb:37-41`) — the class was never given one. This port supplies
    /// the conventional name rather than reproducing a throw.
    #[must_use]
    pub fn name() -> &'static str {
        "Burningham-Otto Solver"
    }

    /// The solver's description. See [`Self::name`] on upstream's missing
    /// implementation.
    #[must_use]
    pub fn description() -> &'static str {
        "Burningham-Otto Sum-Rates (SR) Solver"
    }

    /// Solve the column — equivalent to upstream's `SolveColumn(input)`
    /// (`SumRates.vb:810-895`).
    ///
    /// # Errors
    ///
    /// Any [`ColumnError`] from validation or [`Self::solve`].
    pub fn solve_column(
        &self,
        input: &ColumnSolverInput,
    ) -> Result<ColumnSolverOutput, ColumnError> {
        input.validate_shape()?;
        let thermo = ColumnThermo::new(input.components.clone(), input.package);
        let profile = self.solve(input, &thermo)?;
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

    /// The sum-rates iteration — upstream's `Solve` (`SumRates.vb:49-808`).
    ///
    /// Unlike the bubble-point solvers this method has **no outer
    /// specification loop**: an absorber's degrees of freedom are fixed by its
    /// feeds and duties, so upstream never root-finds on top of it. The
    /// specifications in `input` are evaluated and reported, not imposed.
    ///
    /// # Parameters
    ///
    /// - `input` — the column definition and starting profile. The temperature
    ///   and vapour-flow estimates matter: upstream recommends constant-molar-
    ///   overflow vapour flows worked up from the bottom, and a linear
    ///   temperature profile between assumed top and bottom values
    ///   (`SumRates.vb:83-88`).
    /// - `thermo` — the property-package bridge.
    ///
    /// # Errors
    ///
    /// - [`ColumnError::NotConverged`] on exhausting `max_iterations`
    ///   (upstream `DCMaxIterationsReached`, line 696).
    /// - [`ColumnError::InvalidProfile`] on a non-finite error function or a
    ///   liquid composition that fails to sum (lines 370-372, 700-705), or a
    ///   temperature that goes non-positive (line 616).
    /// - [`ColumnError::TrivialSolution`] if the converged K-values collapse to
    ///   unity (line 792).
    /// - [`ColumnError::SingularMatrix`] from either tridiagonal solve.
    #[allow(clippy::too_many_lines)]
    pub fn solve(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
    ) -> Result<StageProfile, ColumnError> {
        let nc = input.n_components();
        let n = input.number_of_stages;
        let ns = input.top_index();
        let tol = input.outer_tolerance();
        let maxits = input.max_iterations;

        let p = &input.stage_pressures;
        let f = &input.feed_flows;
        let fcj = &input.feed_compositions;
        let hfj = &input.feed_enthalpies;
        let q = input.stage_heats.clone(); // never updated by sum-rates

        let mut tj = input.stage_temperatures.clone();
        let mut vj = input.vapor_flows.clone();
        let mut lj = input.liquid_flows.clone();
        let vssj = input.vapor_side_draws.clone();
        let lssj = input.liquid_side_draws.clone();
        let mut xc = input.liquid_compositions.clone();
        let mut yc = input.vapor_compositions.clone();
        let mut k = input.k_values.clone();

        // K-value seeding / NaN guard (lines 205-221).
        for i in 0..n {
            for j in 0..nc {
                if !k[i][j].is_finite() || k[i][j] <= 0.0 {
                    k[i][j] = thermo.ideal_k_value(j, tj[i], p[i]);
                }
            }
        }

        let mut t_error: f64;
        let mut comp_error: f64;
        let mut ic: usize = 0;

        loop {
            // step4 — tridiagonal component mass balances (lines 308-355).
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
                    // Upstream clamps a negative component flow to 1e-7 here
                    // (line 367) — note this differs from the bubble-point
                    // solvers, which take the absolute value.
                    lc[i][j] = if xt[i] < 0.0 { 1.0e-7 } else { xt[i] };
                }
            }

            let mut sumx = vec![0.0_f64; n];
            for i in 0..n {
                sumx[i] = lc[i].iter().sum();
                if !sumx[i].is_finite() {
                    return Err(ColumnError::InvalidProfile {
                        stage: i,
                        detail: "failed to update liquid phase composition".into(),
                    });
                }
            }

            // *** The sum-rates step *** (lines 375-378).
            for i in 0..n {
                lj[i] *= sumx[i];
            }

            // Compositions (lines 384-398).
            for i in 0..n {
                let mut sumy = 0.0;
                for j in 0..nc {
                    xc[i][j] = if sumx[i] > 0.0 {
                        lc[i][j] / sumx[i]
                    } else {
                        0.0
                    };
                    yc[i][j] = xc[i][j] * k[i][j];
                    sumy += yc[i][j];
                }
                if sumy > 0.0 {
                    for j in 0..nc {
                        yc[i][j] /= sumy;
                    }
                }
            }

            // Vapour flows from the bottom-referenced total balance
            // (lines 402-422). `sum3_j = Σ_{m>=j} (F_m − U_m − W_m)`.
            let mut sum3 = vec![0.0_f64; n];
            let mut running = 0.0_f64;
            for i in (0..n).rev() {
                running += f[i] - lssj[i] - vssj[i];
                sum3[i] = running;
            }
            for i in 0..n {
                vj[i] = if i > 0 {
                    lj[i - 1] - lj[ns] + sum3[i]
                } else {
                    -lj[ns] + sum3[i]
                };
                if vj[i] < 0.0 {
                    vj[i] = -vj[i];
                }
            }

            // Enthalpies and their temperature derivatives (lines 449-560).
            let mut hl = vec![0.0_f64; n];
            let mut hv = vec![0.0_f64; n];
            let mut dhl_dt = vec![0.0_f64; n];
            let mut dhv_dt = vec![0.0_f64; n];
            for i in 0..n {
                hl[i] = thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]);
                hv[i] = thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]);
                let hl_lo = thermo.liquid_molar_enthalpy(&xc[i], tj[i] - DHDT_EPSILON, p[i]);
                let hl_hi = thermo.liquid_molar_enthalpy(&xc[i], tj[i] + DHDT_EPSILON, p[i]);
                let hv_lo = thermo.vapor_molar_enthalpy(&yc[i], tj[i] - DHDT_EPSILON, p[i]);
                let hv_hi = thermo.vapor_molar_enthalpy(&yc[i], tj[i] + DHDT_EPSILON, p[i]);
                dhl_dt[i] = (hl_hi - hl_lo) / (2.0 * DHDT_EPSILON);
                dhv_dt[i] = (hv_hi - hv_lo) / (2.0 * DHDT_EPSILON);
            }

            // Stage energy-balance residuals (lines 549-556).
            let mut h_res = vec![0.0_f64; n];
            for i in 0..n {
                let inflow_liq = if i > 0 { lj[i - 1] * hl[i - 1] } else { 0.0 };
                let inflow_vap = if i < ns { vj[i + 1] * hv[i + 1] } else { 0.0 };
                h_res[i] = inflow_liq + inflow_vap + f[i] * hfj[i]
                    - (lj[i] + lssj[i]) * hl[i]
                    - (vj[i] + vssj[i]) * hv[i]
                    - q[i];
            }

            // Tridiagonal Jacobian ∂H_j/∂T (lines 565-589).
            let mut ath = vec![0.0_f64; n];
            let mut bth = vec![0.0_f64; n];
            let mut cth = vec![0.0_f64; n];
            let mut dth = vec![0.0_f64; n];
            for i in 0..n {
                dth[i] = -h_res[i];
                bth[i] = -(lj[i] + lssj[i]) * dhl_dt[i] - (vj[i] + vssj[i]) * dhv_dt[i];
                if i < ns {
                    cth[i] = vj[i + 1] * dhv_dt[i + 1];
                }
                if i > 0 {
                    ath[i] = lj[i - 1] * dhl_dt[i - 1];
                }
            }
            let delta_t = tdma_solve(&ath, &bth, &cth, &dth)?;

            // Temperature update (lines 595-617). NOTE: upstream computes a
            // step cap `af = 5/max|ΔT|` and never applies it; see the module
            // header.
            let dft = damping_factor(ic, 50, 0.25, 1.0);
            let tj_ant = tj.clone();
            t_error = 0.0;
            comp_error = 0.0;
            for i in 0..n {
                tj[i] += 0.7 * delta_t[i];
                if self.relax_temperature_updates {
                    tj[i] = dft * tj[i] + (1.0 - dft) * tj_ant[i];
                }
                if !(tj[i] > 0.0) || !tj[i].is_finite() {
                    return Err(ColumnError::InvalidProfile {
                        stage: i,
                        detail: format!("converged to an invalid temperature (T = {} K)", tj[i]),
                    });
                }
            }

            // K-values and vapour compositions at the new temperatures
            // (lines 618-666).
            for i in 0..n {
                let k_new = thermo.k_values(&xc[i], &yc[i], tj[i], p[i]);
                let mut sumy = 0.0;
                for j in 0..nc {
                    k[i][j] = k_new[j];
                    let y_ant = yc[i][j];
                    yc[i][j] = k[i][j] * xc[i][j];
                    sumy += yc[i][j];
                    comp_error += (yc[i][j] - y_ant).powi(2);
                }
                t_error += (tj[i] - tj_ant[i]).powi(2);
                if sumy > 0.0 {
                    for j in 0..nc {
                        yc[i][j] /= sumy;
                    }
                }
            }

            ic += 1;

            if ic >= maxits {
                return Err(ColumnError::NotConverged {
                    iterations: ic,
                    error: t_error + comp_error,
                });
            }
            if !t_error.is_finite() || !comp_error.is_finite() {
                return Err(ColumnError::InvalidProfile {
                    stage: 0,
                    detail: "sum-rates error function went non-finite".into(),
                });
            }
            if t_error <= tol && comp_error <= tol && ic > MIN_ITERATIONS {
                break;
            }
        }

        // Trivial-solution guard (lines 790-794).
        for (i, ki) in k.iter().enumerate() {
            if ColumnThermo::is_trivial_solution(ki) {
                return Err(ColumnError::TrivialSolution { stage: i });
            }
        }

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
            error: t_error,
        })
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the sum-rates support routines
    //!
    //! **Methodology.** [`damping_factor`] is checked against the closed form
    //! upstream implements (`DWSIM.Math/Interpolation.vb:39-48`): a linear ramp
    //! `(max − min)/count · current`, clamped. Solver-level V&V lives in
    //! [`crate::columns`]'s integration tests, which run this solver on a real
    //! column.
    //!
    //! **Results (2026-08-11, release build):** passes.

    use super::*;

    /// **Methodology.** `GetDampingFactor(current, 50, 0.25, 1.0)` must clamp to
    /// 0.25 for `current <= 16` (`0.75/50 · 16 = 0.24 < 0.25`), rise linearly
    /// thereafter, and clamp to 1.0 from `current >= 67` (`0.75/50 · 67 = 1.005`).
    /// **Result (2026-08-11):** `f(0) = 0.25`, `f(17) = 0.255`, `f(50) = 0.75`,
    /// `f(100) = 1.0` — matches the closed form exactly.
    #[test]
    fn damping_factor_matches_upstream_ramp() {
        assert!((damping_factor(0, 50, 0.25, 1.0) - 0.25).abs() < 1e-12);
        assert!((damping_factor(17, 50, 0.25, 1.0) - 0.255).abs() < 1e-12);
        assert!((damping_factor(50, 50, 0.25, 1.0) - 0.75).abs() < 1e-12);
        assert!((damping_factor(100, 50, 0.25, 1.0) - 1.0).abs() < 1e-12);
        // Degenerate count must not divide by zero.
        assert!((damping_factor(3, 0, 0.25, 1.0) - 1.0).abs() < 1e-12);
    }
}
