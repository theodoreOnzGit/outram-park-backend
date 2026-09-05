//! Modified Wang-Henke bubble-point (MBP) method — DWSIM's second, newer
//! bubble-point implementation.
//!
//! Pure-Rust port of DWSIM's
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumnSolvers/BubblePoint2.vb`
//! (GPL-3.0), class `WangHenkeMethod2`, upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al.
//!
//! | This module | Upstream member | `BubblePoint2.vb` lines |
//! |---|---|---|
//! | [`ModifiedWangHenkeSolver::solve`] | `Public Function Solve` | 51-714 |
//! | [`run_tridiagonal`] | `Public Shared Function RunTridiagonal` | 716-1172 |
//! | [`ModifiedWangHenkeSolver::solve_internal`] | `Public Function Solve_Internal` | 1174-2160 |
//! | [`ModifiedWangHenkeSolver::solve_column`] | `Public Overrides Function SolveColumn` | 2162-2325 |
//!
//! # Why both bubble-point solvers are ported
//!
//! `BubblePoint2.vb` is not a replacement for `BubblePoint.vb` — DWSIM ships
//! **both** as separately selectable solvers ("Wang-Henke Solver" and "Modified
//! Wang-Henke Solver"), and they converge differently on the same column. The
//! maintainer's instruction for this workstream was explicit: port what is
//! there, do not discard code because it looks redundant. See
//! [`crate::columns::bubble_point`] for the original.
//!
//! # What actually differs from [`crate::columns::bubble_point`]
//!
//! Verified by diffing the two upstream files (`Solve_Internal`
//! `BubblePoint.vb:763-1860` against `BubblePoint2.vb:1174-2160`):
//!
//! 1. **No `Mode` argument, no Broyden temperature branch.** The modified
//!    solver *always* updates temperatures by a per-stage bubble-point
//!    calculation; the whole `Mode = 1` path (`BubblePoint.vb:1328-1417`) and
//!    the `K_max/K_min > 10000` wide-boiling switch are gone. Consequently the
//!    outer loop has no `Try`/`Catch` alt-mode retry cascade either.
//! 2. **Different temperature damping test.** `If maxDT < 10` instead of
//!    `If maxDT < 50` (line 1711 vs `BubblePoint.vb:1312`), and the accepted
//!    branch applies the **undamped** step `T_j = T_ant,j + ΔT_j` rather than
//!    `T_ant,j + af·ΔT_j`. Since `maxDT` is initialised to `50.0` in both files
//!    and this solver never lowers it, the test is always false in practice and
//!    the update is the 50 % average `T = (T_new + T_old)/2` — the same
//!    effective behaviour as the original. Ported faithfully, including the
//!    dead branch, because the *constant* differs and a future upstream change
//!    to `maxDT` would make it live.
//! 3. **Tighter convergence gate.** `Loop Until (t_error + vf_error) < tolerance`
//!    (line 1934 area) rather than `< tolerance * ns / 100`
//!    (`BubblePoint.vb:1789`). For a 12-stage column at `tol = 1e-5` that is a
//!    **110x tighter** criterion — this is the substantive numerical difference
//!    between the two solvers.
//! 4. **A public single-pass helper**, `RunTridiagonal`, ported as
//!    [`run_tridiagonal`]: one tridiagonal + energy-balance sweep with **no**
//!    iteration and **no** temperature update, used to manufacture a consistent
//!    starting profile. Its vapour update also omits the stage-efficiency
//!    blending (`y = K x` unconditionally, lines 979-987), which the iterating
//!    loops apply.
//! 5. Upstream calls `pp.FlashBase.Flash_PV` directly rather than a per-stage
//!    cloned flash algorithm — a threading detail with no numerical effect, and
//!    irrelevant to this serial port.
//!
//! Everything else — the tridiagonal assembly, the spec handling, the energy
//! balance, the liquid-flow update, the duty back-calculation and the
//! mass-balance guard — is line-for-line identical between the two files, and
//! this port shares those helpers with [`crate::columns::bubble_point`] rather
//! than duplicating them.
//!
//! # Units
//!
//! Documented raw `f64` in SI: `T` \[K\], `P` \[Pa\], flows \[mol/s\], molar
//! enthalpies \[J/mol\], duties \[W\], compositions and K-values \[-\].
//!
//! # Excluded DWSIM behavior
//!
//! The same exclusions as [`crate::columns::bubble_point`]: `Inspector`
//! paragraphs and the convergence-report `StringBuilder`, `Parallel.For`
//! branches, flowsheet UI messaging/cancellation, the liquid-liquid extractor
//! mode, unit conversion of spec values, and the Brent fallbacks in the outer
//! reboiler loop.

use crate::columns::bubble_point::{
    back_calculate_duties, impose_condenser_spec, impose_reboiler_spec, net_flow_sums,
};
use crate::columns::linalg::{broyden_root, RootFindOptions};
use crate::columns::model::{
    ColumnError, ColumnSolverInput, ColumnSolverOutput, ColumnSpec, ColumnType, CondenserType,
    SpecType,
};
use crate::columns::profile::StageProfile;
use crate::columns::specs::{evaluate_condenser_spec, evaluate_reboiler_spec, scaled_spec_value};
use crate::columns::thermo_bridge::ColumnThermo;
use crate::columns::tridiagonal::tdma_solve;

/// The modified Wang-Henke bubble-point solver — upstream's
/// `WangHenkeMethod2`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModifiedWangHenkeSolver {
    /// Condenser sub-cooling \[K\] below the bubble point (0 for a saturated
    /// condenser), applied as `T_0 -= ΔT_sub` after the bubble-point update.
    pub subcooling_delta_t: f64,
}

impl ModifiedWangHenkeSolver {
    /// The solver's display name — upstream's `Name` property
    /// (`BubblePoint2.vb:39-43`).
    #[must_use]
    pub fn name() -> &'static str {
        "Modified Wang-Henke Solver"
    }

    /// The solver's description — upstream's `Description` property
    /// (`BubblePoint2.vb:45-49`).
    #[must_use]
    pub fn description() -> &'static str {
        "Modified Wang-Henke Bubble-Point (MBP) Solver"
    }

    /// Solve the column — equivalent to upstream's `SolveColumn(input)`
    /// (`BubblePoint2.vb:2162-2325`).
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
    /// (`BubblePoint2.vb:51-714`).
    ///
    /// Structurally identical to [`crate::columns::bubble_point::WangHenkeSolver::solve`]
    /// minus the alt-mode retries: four paths chosen by whether each user
    /// specification can be imposed directly on the inner loop's mass balance.
    ///
    /// # Errors
    ///
    /// [`ColumnError::NotConverged`] if the outer root-find cannot meet the
    /// tolerance, or any inner-loop error.
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
        let tol = input.inner_tolerance();
        let sum_f: f64 = input.feed_flows.iter().sum();

        if spec_c_direct && spec_r_direct {
            return self.solve_internal(input, thermo, cspec, rspec, input.early_stop_iteration);
        }

        // Surrogate specs: reflux ratio at the condenser, bottoms molar rate at
        // the reboiler (upstream lines 148-158, 347-353, 523-528).
        let rr0 = match cspec.initial_estimate {
            Some(v) if v.is_finite() && v > 0.0 => v,
            _ => {
                let l0 = input.liquid_flows[0];
                let lss0 = input.liquid_side_draws[0];
                if input.condenser_type != CondenserType::FullReflux && lss0 != 0.0 {
                    (l0 + lss0) / lss0
                } else {
                    2.0
                }
            }
        };
        let b0 = *input.liquid_flows.last().expect("validated non-empty");

        let mut best: Option<(f64, StageProfile)> = None;
        let mut x0: Vec<f64> = Vec::new();
        if !spec_c_direct {
            x0.push(rr0);
        }
        if !spec_r_direct {
            x0.push(b0);
        }

        let mut residual = |xv: &[f64]| -> Option<Vec<f64>> {
            let mut idx = 0;
            let cs = if spec_c_direct {
                cspec.clone()
            } else {
                let s = ColumnSpec::reflux_ratio(xv[idx].abs());
                idx += 1;
                s
            };
            let rs = if spec_r_direct {
                rspec.clone()
            } else {
                let mut b = xv[idx].abs();
                if b > sum_f {
                    b = 0.99 * sum_f;
                }
                ColumnSpec {
                    spec_type: SpecType::ProductMolarFlowRate,
                    value: b,
                    ..ColumnSpec::default()
                }
            };
            let profile = self
                .solve_internal(input, thermo, &cs, &rs, input.early_stop_iteration)
                .ok()?;
            let mut errs = Vec::new();
            if !spec_c_direct {
                errs.push(
                    evaluate_condenser_spec(
                        cspec,
                        &profile,
                        thermo,
                        &input.feed_flows,
                        &input.overall_compositions,
                        input.condenser_type,
                    )
                    .ok()?
                    .error?,
                );
            }
            if !spec_r_direct {
                errs.push(
                    evaluate_reboiler_spec(
                        rspec,
                        &profile,
                        thermo,
                        &input.feed_flows,
                        &input.overall_compositions,
                    )
                    .ok()?
                    .error?,
                );
            }
            let obj: f64 = errs.iter().map(|e| e * e).sum();
            if best.as_ref().is_none_or(|(b2, _)| obj < *b2) {
                best = Some((obj, profile));
            }
            Some(errs)
        };

        let r = broyden_root(
            &mut residual,
            &x0,
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

    /// The modified inner loop — upstream's `Solve_Internal`
    /// (`BubblePoint2.vb:1174-2160`).
    ///
    /// Identical to
    /// [`crate::columns::bubble_point::WangHenkeSolver::solve_internal`] except
    /// for differences 1-3 listed in the module header: no Broyden temperature
    /// mode, the `maxDT < 10` damping test, and the untightened `< tolerance`
    /// convergence gate.
    ///
    /// # Parameters / Errors
    ///
    /// As
    /// [`crate::columns::bubble_point::WangHenkeSolver::solve_internal`], minus
    /// the `mode` argument.
    #[allow(clippy::too_many_lines)]
    pub fn solve_internal(
        &self,
        input: &ColumnSolverInput,
        thermo: &ColumnThermo,
        cspec: &ColumnSpec,
        rspec: &ColumnSpec,
        stop_at: Option<usize>,
    ) -> Result<StageProfile, ColumnError> {
        let nc = input.n_components();
        let n = input.number_of_stages;
        let ns = input.top_index();
        let tolerance = input.inner_tolerance();
        let maxits = input.max_iterations;

        let rebabs = input.column_type == ColumnType::ReboiledAbsorber;
        let refabs = input.column_type == ColumnType::RefluxedAbsorber;
        let condt = input.condenser_type;

        let spval1 = scaled_spec_value(cspec);
        let spval2 = scaled_spec_value(rspec);

        let p = &input.stage_pressures;
        let f = &input.feed_flows;
        let fcj = &input.feed_compositions;
        let eff = &input.stage_efficiencies;
        let hfj = &input.feed_enthalpies;

        let mut tj = input.stage_temperatures.clone();
        let mut vj = input.vapor_flows.clone();
        let mut lj = input.liquid_flows.clone();
        let vssj = input.vapor_side_draws.clone();
        let mut lssj = input.liquid_side_draws.clone();
        let mut k = input.k_values.clone();
        let mut k_ant = k.clone();
        let mut q = input.stage_heats.clone();
        let mut xc = input.liquid_compositions.clone();
        let mut yc = input.vapor_compositions.clone();

        let mut hl: Vec<f64> = (0..n)
            .map(|i| thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]))
            .collect();
        let mut hv: Vec<f64> = (0..n)
            .map(|i| thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]))
            .collect();

        let mut sum_f: f64 = f.iter().sum();
        let mut sum_lss: f64 = input.liquid_side_draws.iter().skip(1).sum();
        let mut sum_vss: f64 = vssj.iter().sum();

        let mut rr: f64;
        let _pre_loop_rr = impose_condenser_spec(
            cspec, spval1, rebabs, thermo, &xc, &mut lssj, &mut q, &lj, &vj, &vssj, f, hfj, &hl,
            &hv, sum_f,
        );
        let mut b;
        b = impose_reboiler_spec(
            rspec, spval2, refabs, thermo, &xc, &lssj, &vssj, &lj, &vj, f, hfj, &hl, &hv, &mut q,
            sum_f, ns,
        );

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

        // Upstream initialises this to 50.0 and never lowers it in this solver,
        // so the `< 10` branch below is dead in practice (module header, #2).
        let max_dt = 50.0_f64;
        let mut t_error: f64;
        let mut vf_error: f64;
        let mut ic: usize = 0;

        loop {
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
                    lc[i][j] = xt[i].abs();
                }
            }

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

            // Temperature update — always the bubble-point calculation.
            let tj_ant = tj.clone();
            for i in 0..n {
                let (t_new, k_new) = thermo.bubble_temperature(&xc[i], p[i], tj[i], i)?;
                k_ant[i].clone_from(&k[i]);
                if t_new > 0.0 && t_new.is_finite() {
                    tj[i] = t_new;
                    k[i] = k_new;
                }
            }
            tj[0] -= self.subcooling_delta_t;

            let d_tj: Vec<f64> = (0..n).map(|i| tj[i] - tj_ant[i]).collect();
            if max_dt < 10.0 {
                // Upstream applies the *undamped* step here (line 1713).
                for i in 0..n {
                    let t_new = tj_ant[i] + d_tj[i];
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

            for i in 0..n {
                hl[i] = thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]);
                hv[i] = thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]);
            }

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
            // Difference #3: no `* ns / 100` relaxation.
            if (t_error + vf_error) < tolerance && ic > 1 {
                break;
            }
        }

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

/// One non-iterating tridiagonal + energy-balance sweep — upstream's
/// `RunTridiagonal` (`BubblePoint2.vb:716-1172`).
///
/// Takes the supplied `(T, V, L, K)` estimate, solves the component mass
/// balances tridiagonally at that `(T, V)`, forms `y = K x` (**without** the
/// stage-efficiency blend the iterating loops apply — lines 979-987), refreshes
/// the enthalpies, imposes both specifications, and runs the energy balance
/// once to update `V`, `L` and the end duties. **Temperatures are never
/// updated**: this is a consistency pass, not a solve.
///
/// Upstream exposes it as a `Public Shared Function` so the column object can
/// manufacture a self-consistent starting profile before handing it to a
/// rigorous solver; it is also useful on its own for checking whether an
/// initial estimate is even mass-consistent.
///
/// # Parameters
///
/// - `input` — the column definition and the starting profile to sweep.
/// - `thermo` — the property-package bridge.
/// - `cspec` / `rspec` — the specifications to impose.
///
/// # Returns
///
/// A [`StageProfile`] with `iterations = 1` and `error = f64::NAN` (upstream
/// returns no error term from this function — it does not compute one).
///
/// # Errors
///
/// [`ColumnError::SingularMatrix`] from the tridiagonal solve, or
/// [`ColumnError::LengthMismatch`] / [`ColumnError::TooFewStages`] from input
/// validation.
pub fn run_tridiagonal(
    input: &ColumnSolverInput,
    thermo: &ColumnThermo,
    cspec: &ColumnSpec,
    rspec: &ColumnSpec,
) -> Result<StageProfile, ColumnError> {
    input.validate_shape()?;
    let nc = input.n_components();
    let n = input.number_of_stages;
    let ns = input.top_index();

    let rebabs = input.column_type == ColumnType::ReboiledAbsorber;
    let refabs = input.column_type == ColumnType::RefluxedAbsorber;
    let condt = input.condenser_type;

    let spval1 = scaled_spec_value(cspec);
    let spval2 = scaled_spec_value(rspec);

    let p = &input.stage_pressures;
    let f = &input.feed_flows;
    let fcj = &input.feed_compositions;
    let hfj = &input.feed_enthalpies;
    let tj = input.stage_temperatures.clone();
    let mut vj = input.vapor_flows.clone();
    let mut lj = input.liquid_flows.clone();
    let vssj = input.vapor_side_draws.clone();
    let mut lssj = input.liquid_side_draws.clone();
    let mut k = input.k_values.clone();
    let mut q = input.stage_heats.clone();
    let mut xc = input.liquid_compositions.clone();
    let mut yc = input.vapor_compositions.clone();

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
            lc[i][j] = xt[i].abs();
        }
    }

    // Upstream's `ic` is 0 in this function, so the `Else` branch runs and the
    // liquid compositions stay at the supplied estimate (lines 968-973).
    let _ = &lc;

    // y = K x, no efficiency blend (lines 979-991).
    for i in 0..n {
        let mut sumy = 0.0;
        for j in 0..nc {
            yc[i][j] = k[i][j] * xc[i][j];
            sumy += yc[i][j];
        }
        if sumy > 0.0 {
            for j in 0..nc {
                yc[i][j] /= sumy;
            }
        }
    }

    let hl: Vec<f64> = (0..n)
        .map(|i| thermo.liquid_molar_enthalpy(&xc[i], tj[i], p[i]))
        .collect();
    let hv: Vec<f64> = (0..n)
        .map(|i| thermo.vapor_molar_enthalpy(&yc[i], tj[i], p[i]))
        .collect();

    let sum_f: f64 = f.iter().sum();
    let sum_lss: f64 = input.liquid_side_draws.iter().skip(1).sum();
    let sum_vss: f64 = vssj.iter().sum();

    let rr = impose_condenser_spec(
        cspec, spval1, rebabs, thermo, &xc, &mut lssj, &mut q, &lj, &vj, &vssj, f, hfj, &hl, &hv,
        sum_f,
    );
    let b = impose_reboiler_spec(
        rspec, spval2, refabs, thermo, &xc, &lssj, &vssj, &lj, &vj, f, hfj, &hl, &hv, &mut q,
        sum_f, ns,
    );

    if condt == CondenserType::FullReflux || rebabs {
        vj[0] = (sum_f - b - sum_lss - sum_vss).abs();
        lssj[0] = 0.0;
    } else {
        lssj[0] = sum_f - b - sum_lss - sum_vss - vj[0];
    }

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
        if !vj[i].is_finite() || vj[i] < 0.0 {
            vj[i] = 1.0e-10;
        }
    }

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

    // Keep `xc` explicit for the reader: unchanged from the input estimate.
    xc.clone_from(&input.liquid_compositions);

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
        iterations: 1,
        error: f64::NAN,
    })
}
