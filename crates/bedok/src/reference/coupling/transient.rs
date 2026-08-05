//! Transient coupled neutronics/thermal-hydraulics solve.
//!
//! # Provenance
//!
//! Translated from `thdiffusion_solvertimexyz.m` in Than Yan Ren's (SNRSI)
//! BEDOK MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`,
//! received 2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear
//! Research and Safety Institute. Translated with permission; see
//! `docs/bedok-port-scoping.md` §6.
//!
//! # The method, in three phases
//!
//! **Phase 1 — initial steady state.** The static coupled solver
//! ([`super::steady::solve_coupled_steady`]) is run to convergence, and the
//! transient fission operator is then divided by the resulting `k_eff` so the
//! initial state is *exactly* critical. That stands in for the critical-boron
//! search the benchmark performs to the same effect.
//!
//! **Phase 2 — rebuild and re-equilibrate.** The diffusion operator is rebuilt
//! at the converged steady state and the flux and `k_eff` are re-equilibrated
//! on it with a short power iteration, so the transient starts from an exact
//! equilibrium of *the operator actually used in the time stepping* rather than
//! of whatever the eigensolver last held.
//!
//! **Phase 3 — time integration.** The multigroup diffusion equation with six
//! delayed-neutron precursor families, the prescribed control-assembly
//! ejection, and one transient T-H step per time step.
//!
//! # Two kinetics schemes
//!
//! [`KineticsScheme::ExponentialTransform`] (the default) is the scheme of the
//! nodal program *Ants* — A. Rintala and U. Lauranto, *Ann. Nucl. Energy* **190**
//! (2023) 109868, Eqs. (3)–(13): implicit Euler on an exponentially transformed
//! flux, with the precursors integrated analytically under the assumption that
//! the transformed fission source varies linearly over the step. The
//! frequencies are iterated **within** the step — a predictor pass at
//! `omega = 0`, then `freq_iter - 1` correctors using the newest flux of the
//! current step, which is the remark under the paper's Eq. (4). Yan Ren records
//! that extrapolating the frequencies from the *previous* step instead proved
//! unstable against the lagged T-H feedback (a growing two-step power
//! oscillation), so it is not used.
//!
//! [`KineticsScheme::ImplicitEuler`] is the legacy first-order scheme with the
//! precursors eliminated analytically per step.
//!
//! # There is no Newton-Krylov solve here
//!
//! Each time step is a **linear** system solved directly (sparse LU). The
//! feedback is closed by Picard passes (`params.timepicard`), not by a Newton
//! iteration. See the note in [`super`] on the snapshot's dead `params.jfnk*`
//! controls.

use super::cross_section_feedback::update_cross_sections;
use super::error::{CouplingError, Result};
use super::seam::{
    self, CaseParams, CoreGeometry, FrequencyMode, KineticsScheme, MaterialMap, NodalTerms,
    SigmaValues, ThermalState,
};
use super::sparse::{
    add_diagonal, fix_inf_nan, linear_combination, norm1, norm2, scale_columns, spmv, sum,
    SparseLu, SparseMatrix,
};
use super::steady::{solve_coupled_steady, SteadyOutput};

/// Default output-file prefix, MATLAB `params.outprefix`.
pub const DEFAULT_OUT_PREFIX: &str = "neacrpa2t";

/// Default number of flux solves per step under the exponential transform:
/// one predictor plus one frequency corrector.
pub const DEFAULT_FREQ_ITER: usize = 2;

/// Refinement passes of the nodal correction at the fixed converged steady flux
/// in Phase 2 — the MATLAB's initial call plus four more.
pub const PHASE2_NODAL_REFINEMENTS: usize = 4;

/// Maximum power iterations in the Phase-2 re-equilibration.
///
/// Yan Ren's note: *"heavily rodded cores have a high dominance ratio — allow
/// many cheap triangular-solve iterations rather than exiting unconverged"*.
pub const PHASE2_MAX_POWER_ITERATIONS: usize = 5000;

/// Fission-source and `k_eff` tolerance of the Phase-2 re-equilibration \[-\].
pub const PHASE2_TOL: f64 = 1.0e-9;

/// Divergence guard on the relative power \[-\].
///
/// Deliberately far above any physical excursion: hot-zero-power cases start
/// at `P0 ~ kW`, so `P/P0 ~ 1e6` is real physics, and only `> 1e12` is taken
/// as a blown-up solution.
pub const DIVERGENCE_POWER_RATIO: f64 = 1.0e12;

/// Lower clamp on the per-step exponent `omega*dt` \[-\].
///
/// A **physics** bound rather than overflow protection: keeping
/// `omega*dt >= -0.9` also keeps the transformed time-derivative coefficient
/// `omega + 1/dt` positive.
pub const OMEGA_DT_MIN: f64 = -0.9;

/// Upper clamp on the per-step exponent `omega*dt` \[-\], i.e. at most a
/// factor `e^2 ≈ 7.4` growth per step. Keeps the transform effective for the
/// global mode while bounding pathological extrapolation.
pub const OMEGA_DT_MAX: f64 = 2.0;

/// Result of a transient coupled solve — the MATLAB `output` struct of
/// `thdiffusion_solvertimexyz.m`.
///
/// The `C1`–`C6` labels are the reported quantities of NEACRP-L-335 section 4C.
#[derive(Debug, Clone)]
pub struct TransientOutput {
    /// Initial (re-equilibrated) multiplication factor \[-\].
    pub k_eff: f64,
    /// The Phase-1 steady state, returned whole.
    pub steady: SteadyOutput,
    /// Final transient thermal-hydraulic state.
    pub th: ThermalState,

    /// Time points \[s\], truncated at the divergence guard if it tripped.
    pub time: Vec<f64>,
    /// **C1** — core power relative to its steady value \[-\].
    pub relative_power: Vec<f64>,
    /// **C2** — core-average fuel temperature \[K\].
    pub avg_fuel_temp: Vec<f64>,
    /// **C3** — maximum fuel temperature \[K\].
    pub max_fuel_temp: Vec<f64>,
    /// **C4** — core-average coolant outlet temperature \[K\].
    pub coolant_outlet_temp: Vec<f64>,
    /// **C5-1** — radial power map at active-core axial layer 6, at the time of
    /// the power maximum, normalised to a peak of 1. `nx*ny`, indexed
    /// `ix*ny + iy`.
    pub radial_c5_z6: Vec<f64>,
    /// **C5-2** — the same at active-core axial layer 13.
    pub radial_c5_z13: Vec<f64>,
    /// **C6-1** — radial power map at layer 6 at the final time.
    pub radial_c6_z6: Vec<f64>,
    /// **C6-2** — the same at layer 13.
    pub radial_c6_z13: Vec<f64>,

    /// Time of the power maximum \[s\].
    pub t_power_max: f64,
    /// Peak relative power \[-\].
    pub relative_power_max: f64,
    /// Ejected-bank position per time step \[steps withdrawn\].
    pub rod_position: Vec<f64>,
    /// Final scalar flux, `philenf` entries.
    pub scalar_flux_final: Vec<f64>,
    /// Final group-collapsed node power \[W\], `nodes` entries.
    pub pwrdens_final: Vec<f64>,
    /// Final precursor concentrations, `n_families` vectors of `philenf`
    /// entries each.
    pub precursors_final: Vec<Vec<f64>>,
    /// Which kinetics scheme was used.
    pub time_scheme: KineticsScheme,
    /// Whether the divergence guard tripped.
    pub diverged: bool,
}

/// Solve the coupled transient.
///
/// MATLAB `thdiffusion_solvertimexyz(geometry, params, th, sigmavalues,
/// whichsigma, initial_k_eff)`.
///
/// # Case data the transient needs
///
/// From `params`: [`velocities`](CaseParams::velocities) \[cm/s\],
/// [`beta_dnp`](CaseParams::beta_dnp) \[-\],
/// [`lambda_dnp`](CaseParams::lambda_dnp) \[1/s\],
/// [`t_end`](CaseParams::t_end) and/or [`t_grid`](CaseParams::t_grid) \[s\],
/// and [`eject_duration`](CaseParams::eject_duration) \[s\] when a bank moves.
/// From `geometry`: [`crod_eject`](CoreGeometry::crod_eject) and
/// [`crod_eject_to`](CoreGeometry::crod_eject_to).
///
/// # Time grid
///
/// `[0, tgrid…, tend]`, rounded to 1 µs and deduplicated so overlapping range
/// endpoints cannot produce a near-zero step, then truncated at `tend`.
///
/// # Deviations from the MATLAB
///
/// - **No file output.** The MATLAB writes `<prefix>_C1toC4_history.csv` and
///   four `C5`/`C6` matrices, and optionally a JPEG plot. Everything in them is
///   returned in [`TransientOutput`] instead.
/// - **No steady-state cache.** `params.steadyfile` names a `.mat` file the
///   MATLAB loads if present and writes otherwise. There is no `.mat` support
///   here, so the steady solve always runs;
///   [`CaseParams::steady_file`] is carried but not acted on.
/// - **No progress printing.**
/// - **No per-step wall times.** The MATLAB records `output.steptime` (and
///   prints a mean) purely as a performance diagnostic; it is not reproduced.
///
/// None of the four changes a number.
///
/// # Errors
///
/// [`CouplingError::NoTimeData`] if the case sets neither `tend` nor `tgrid`;
/// [`CouplingError::MissingCaseData`] if a bank ejection is declared without an
/// ejection duration or target; plus any sparse or feedback failure.
///
/// # Panics
///
/// Through the [`seam`] stubs until `nodal/` and `th/` land.
#[allow(clippy::too_many_lines)]
pub fn solve_coupled_transient(
    geometry: &CoreGeometry,
    params: &CaseParams,
    th_in: &ThermalState,
    sigma_values: &SigmaValues,
    which_sigma: &MaterialMap,
    initial_k_eff: Option<f64>,
) -> Result<TransientOutput> {
    let grid = params.grid;
    let nodes = grid.nodes();
    let ngroups = grid.ngroups;
    let philen = grid.state_len();
    let philenf = philen + params.n_components * nodes;

    let vi_per_group = seam::replicate_per_group(&grid, &geometry.base.volume);

    // ----- kinetics data ----- //
    let velocities = &params.velocities;
    let beta = &params.beta_dnp;
    let lambda = &params.lambda_dnp;
    let beta_total: f64 = beta.iter().sum();
    let n_dnp = beta.len();

    // ----- transient controls ----- //
    let t_end = match (params.t_end, params.t_grid.as_ref()) {
        (Some(t), _) => t,
        (None, Some(g)) => g.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        (None, None) => return Err(CouplingError::NoTimeData),
    };
    let t_grid = build_time_grid(params.t_grid.as_deref(), t_end);
    let mut n_t = t_grid.len();

    let n_picard = params.time_picard.unwrap_or(1);
    let nodal_upd_time = params.nodal_upd_time.unwrap_or(1);
    let time_scheme = params.time_scheme.unwrap_or_default();
    let n_freq = params.freq_iter.unwrap_or(DEFAULT_FREQ_ITER).max(1);
    let freq_mode = params.freq_mode.unwrap_or_default();

    // Control-assembly ejection. Optional: cases with no rod motion (the BWR D1
    // inlet cold-water transient) set `geometry.crodeject = 0` or omit it.
    let eject_bank = geometry.crod_eject.filter(|b| *b > 0);
    let (eject_from, eject_to, eject_duration) = match eject_bank {
        Some(bank) => {
            let to = geometry
                .crod_eject_to
                .ok_or(CouplingError::MissingCaseData {
                    what: "geometry.crodejectto, required when a bank is ejected",
                })?;
            let duration = params
                .eject_duration
                .ok_or(CouplingError::MissingCaseData {
                    what: "params.ejectduration, required when a bank is ejected",
                })?;
            (geometry.crod[bank - 1], to, duration)
        }
        None => (0.0, 0.0, 0.0),
    };

    let sigma_values_ref = sigma_values;
    let which_sigma_ref = which_sigma;

    // =================================================================== //
    // ----- Phase 1: initial steady state (static coupled solver) ----- //
    // =================================================================== //

    let steady = solve_coupled_steady(
        geometry,
        params,
        th_in,
        sigma_values_ref,
        which_sigma_ref,
        initial_k_eff,
    )?;

    let mut phi = steady.scalar_flux.clone();
    let mut th = steady.th.clone();
    let mut k0 = steady.k_eff;
    let power_ratio_0 = th.power_ratio;

    // ============================================================ //
    // ----- Phase 2: rebuild operators and re-equilibrate ----- //
    // ============================================================ //

    // Local copy of the geometry carrying the moving CA position.
    let mut geom_t = geometry.clone();

    let (sigma_values_t, which_sigma_t) =
        update_cross_sections(params, &geom_t, sigma_values_ref, which_sigma_ref, &th)?;
    let mut sigma = seam::make_sigma_operators(params, &sigma_values_t, &which_sigma_t);
    let diffusion = seam::calc_diffusion_coefficients(params, &sigma_values_t.tot, &which_sigma_t);
    let (grad_d, gradient_terms) =
        seam::make_gradient_diffusion_operator(&geom_t, params, &diffusion, &which_sigma_t);
    geom_t.nodal_coeffs = seam::calc_nodal_coefficients(params, &geom_t, &sigma, &diffusion);

    let mut nodal_terms = NodalTerms::zeros(philen);
    let mut nodal;
    {
        let (n, nt) = seam::calc_semi_analytic_nodal(
            params,
            &geom_t,
            &phi,
            &sigma,
            &diffusion,
            &gradient_terms,
            &nodal_terms,
            k0,
        );
        nodal = n;
        nodal_terms = nt;
    }
    // Refine the nodal correction at the (fixed) converged flux.
    for _ in 0..PHASE2_NODAL_REFINEMENTS {
        let (n, nt) = seam::calc_semi_analytic_nodal(
            params,
            &geom_t,
            &phi,
            &sigma,
            &diffusion,
            &gradient_terms,
            &nodal_terms,
            k0,
        );
        nodal = n;
        nodal_terms = nt;
    }
    let mut m_operator = diffusion_operator(&grad_d, &nodal, &sigma.tot, &sigma.s)?;

    // Short power iteration so (phi, k0) is an exact equilibrium of M.
    {
        let lu = SparseLu::factorise(&m_operator)?;
        let mut fs = spmv(&sigma.f, &phi);
        let fs_norm_0 = sum(&fs);
        for _ in 0..PHASE2_MAX_POWER_ITERATIONS {
            let rhs: Vec<f64> = fs.iter().map(|v| v / k0).collect();
            let mut phi_new = lu.solve(&rhs);
            fix_inf_nan(&mut phi_new);
            let mut fs_new = spmv(&sigma.f, &phi_new);
            let k0_new = k0 * norm1(&fs_new) / norm1(&fs);
            let scale = fs_norm_0 / sum(&fs_new);
            for v in &mut phi_new {
                *v *= scale;
            }
            for v in &mut fs_new {
                *v *= scale;
            }
            let difference: Vec<f64> = fs_new.iter().zip(fs.iter()).map(|(a, b)| a - b).collect();
            let residual = norm2(&difference) / norm2(&fs);
            let k_residual = (k0_new - k0).abs() / k0;
            phi = phi_new;
            fs = fs_new;
            k0 = k0_new;
            if residual < PHASE2_TOL && k_residual < PHASE2_TOL {
                break;
            }
        }
    }

    // ----- inverse velocity vector (zero on void nodes) ----- //
    let mut inv_v = vec![0.0_f64; philenf];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                if which_sigma_ref.at(ix, iy, iz) == 0 {
                    continue;
                }
                for g in 0..ngroups {
                    inv_v[grid.index(g, ix, iy, iz)] = 1.0 / velocities[g];
                }
            }
        }
    }

    // ----- initial precursor concentrations (equilibrium) ----- //
    let fs_equilibrium = spmv(&sigma.f, &phi);
    let mut precursors: Vec<Vec<f64>> = (0..n_dnp)
        .map(|i| {
            let mut c = vec![0.0_f64; philenf];
            for (slot, &f) in c.iter_mut().zip(fs_equilibrium.iter()) {
                *slot = beta[i] * f / (lambda[i] * k0);
            }
            c
        })
        .collect();

    // ----- initial power ----- //
    let power_0 = {
        let fp_phi = spmv(&sigma.fp, &phi);
        fp_phi
            .iter()
            .zip(vi_per_group.iter())
            .map(|(p, v)| p * v)
            .sum::<f64>()
    };

    // ----- output bookkeeping ----- //
    let fuel_mask = fuel_node_mask(&grid, which_sigma_ref);
    let outlet_index = channel_outlet_indices(&grid, &fuel_mask, &geom_t.zhis);
    let radial_weights = fuel_radial_weights(params, geometry);
    let fuel_volume: Vec<f64> = geometry
        .base
        .volume
        .iter()
        .zip(fuel_mask.iter())
        .map(|(v, m)| v * m)
        .collect();

    let mut relative_power = vec![1.0_f64; n_t];
    let mut avg_fuel_temp = vec![0.0_f64; n_t];
    let mut max_fuel_temp = vec![0.0_f64; n_t];
    let mut coolant_outlet_temp = vec![0.0_f64; n_t];
    let mut rod_position = vec![0.0_f64; n_t];

    avg_fuel_temp[0] = core_average_fuel_temperature(&th, &radial_weights, &fuel_volume);
    max_fuel_temp[0] = maximum_fuel_temperature(&th, &fuel_mask, radial_weights.len());
    coolant_outlet_temp[0] = average_coolant_outlet_temperature(&th, &outlet_index);
    rod_position[0] = eject_from;

    let mut pwr_node = {
        let fp_phi = spmv(&sigma.fp, &phi);
        let pwr: Vec<f64> = fp_phi
            .iter()
            .zip(vi_per_group.iter())
            .map(|(p, v)| p * v)
            .collect();
        collapse_power_over_groups(&grid, &pwr)
    };
    let mut pwr_node_at_max = pwr_node.clone();
    let mut relative_power_max = 1.0_f64;
    let mut t_power_max = 0.0_f64;

    // ============================================================ //
    // ----- Phase 3: time integration ----- //
    // ============================================================ //

    // Fission operator of the previous time step (the F0 terms).
    let mut sigma_f_old: SparseMatrix = sigma.f.clone();
    let mut diverged = false;
    let mut last_step = n_t;

    for n in 1..n_t {
        let t = t_grid[n];
        let dt = t - t_grid[n - 1];

        // Prescribed CA ejection: linear over the ejection duration, then fully
        // withdrawn. Skipped when the case has no rod motion.
        if let Some(bank) = eject_bank {
            geom_t.crod[bank - 1] =
                eject_from + (eject_to - eject_from) * (t / eject_duration).min(1.0);
            rod_position[n] = geom_t.crod[bank - 1];
        }

        // Time-dependent inlet forcing (e.g. NEACRP D1 cold-water injection).
        // The transient T-H enthalpy march reads `th.coolant.inlettemp` fresh
        // each step, so updating it here applies the new-time boundary value to
        // the implicit (backward-Euler) step.
        if let Some(schedule) = th.inlet_temp_schedule {
            th.coolant.inlet_temp = schedule.evaluate(t, th.coolant.inlet_temp);
        }

        let phi_old = phi.clone();
        let precursors_old = precursors.clone();
        let th_old = th.clone();

        let mut relative_power_t = 0.0_f64;
        let mut pwr = vec![0.0_f64; philen];

        for _pic in 0..n_picard {
            // --- cross sections / operators at the current rod position and
            //     T-H state ---
            let (sigma_values_t, which_sigma_t) =
                update_cross_sections(params, &geom_t, sigma_values_ref, which_sigma_ref, &th)?;
            sigma = seam::make_sigma_operators(params, &sigma_values_t, &which_sigma_t);
            let diffusion =
                seam::calc_diffusion_coefficients(params, &sigma_values_t.tot, &which_sigma_t);
            let (grad_d, gradient_terms) =
                seam::make_gradient_diffusion_operator(&geom_t, params, &diffusion, &which_sigma_t);
            geom_t.nodal_coeffs =
                seam::calc_nodal_coefficients(params, &geom_t, &sigma, &diffusion);
            if nodal_upd_time > 0 && ((n - 1) % nodal_upd_time == 0 || n_picard > 1) {
                let (new_nodal, new_terms) = seam::calc_semi_analytic_nodal(
                    params,
                    &geom_t,
                    &phi,
                    &sigma,
                    &diffusion,
                    &gradient_terms,
                    &nodal_terms,
                    k0,
                );
                nodal = new_nodal;
                nodal_terms = new_terms;
            }
            m_operator = diffusion_operator(&grad_d, &nodal, &sigma.tot, &sigma.s)?;

            match time_scheme {
                KineticsScheme::ExponentialTransform => {
                    // Carried out of the frequency loop for the precursor
                    // update below, exactly as the MATLAB's `F1` and `del0`
                    // survive their `for fi` loop.
                    let mut f1_coefficients: Vec<Vec<f64>> = vec![vec![0.0; philenf]; n_dnp];
                    let mut lagged_delayed: Vec<Vec<f64>> = vec![vec![0.0; philenf]; n_dnp];

                    for fi in 0..n_freq {
                        let (omega, omega_dt) = if fi == 0 {
                            (vec![0.0_f64; philenf], vec![0.0_f64; philenf])
                        } else {
                            let raw = match freq_mode {
                                FrequencyMode::Node => node_frequencies(&phi, &phi_old, dt, &inv_v),
                                FrequencyMode::Global => global_group_frequencies(
                                    &grid,
                                    philenf,
                                    &phi,
                                    &phi_old,
                                    &vi_per_group,
                                    &inv_v,
                                    dt,
                                ),
                            };
                            let omega_dt: Vec<f64> = raw
                                .iter()
                                .map(|w| (w * dt).clamp(OMEGA_DT_MIN, OMEGA_DT_MAX))
                                .collect();
                            let omega: Vec<f64> = omega_dt.iter().map(|x| x / dt).collect();
                            (omega, omega_dt)
                        };

                        // Precursor coefficients, Eqs. (9)-(10) rewritten with
                        // x = (lambda + omega)*dt:
                        //   F0 = beta*dt*exp(-lambda*dt)*g0(x)
                        //   F1 = beta*dt*g1(x)
                        let mut ff1 = vec![0.0_f64; philenf];
                        // V0 term, Eq. (12).
                        let mut rhs: Vec<f64> = (0..philenf)
                            .map(|i| inv_v[i] * omega_dt[i].exp() * phi_old[i] / dt)
                            .collect();

                        for i in 0..n_dnp {
                            let x: Vec<f64> = omega.iter().map(|w| (lambda[i] + w) * dt).collect();
                            let f0_i: Vec<f64> = x
                                .iter()
                                .map(|xi| beta[i] * dt * (-lambda[i] * dt).exp() * g_exp_0(*xi))
                                .collect();
                            f1_coefficients[i] =
                                x.iter().map(|xi| beta[i] * dt * g_exp_1(*xi)).collect();

                            let f0_phi_old: Vec<f64> = f0_i
                                .iter()
                                .zip(phi_old.iter())
                                .map(|(f0, p)| f0 * p)
                                .collect();
                            let delayed = spmv(&sigma_f_old, &f0_phi_old);
                            lagged_delayed[i] = delayed.iter().map(|d| d / k0).collect();

                            for j in 0..philenf {
                                ff1[j] += lambda[i] * f1_coefficients[i][j];
                                rhs[j] += lambda[i]
                                    * ((-lambda[i] * dt).exp() * precursors_old[i][j]
                                        + lagged_delayed[i][j]);
                            }
                        }

                        // V1 term Eq. (13); the F1 delayed production of the new
                        // flux moves into the system matrix as a column scaling
                        // of the fission operator.
                        let time_derivative: Vec<f64> = (0..philenf)
                            .map(|i| inv_v[i] * (omega[i] + 1.0 / dt))
                            .collect();
                        let ff1_over_k: Vec<f64> = ff1.iter().map(|v| v / k0).collect();
                        let delayed_column_scaled = scale_columns(&sigma.f, &ff1_over_k)?;
                        let lhs = {
                            let with_diagonal = add_diagonal(&m_operator, &time_derivative)?;
                            linear_combination(&[
                                (1.0, &with_diagonal),
                                (-(1.0 - beta_total) / k0, &sigma.f),
                                (-1.0, &delayed_column_scaled),
                            ])?
                        };
                        let lu = SparseLu::factorise(&lhs)?;
                        phi = lu.solve(&rhs);
                        fix_inf_nan(&mut phi);
                    }

                    // --- analytic precursor update, Eq. (8) ---
                    for i in 0..n_dnp {
                        let f1_phi: Vec<f64> = f1_coefficients[i]
                            .iter()
                            .zip(phi.iter())
                            .map(|(f1, p)| f1 * p)
                            .collect();
                        let production = spmv(&sigma.f, &f1_phi);
                        for j in 0..philenf {
                            precursors[i][j] = (-lambda[i] * dt).exp() * precursors_old[i][j]
                                + lagged_delayed[i][j]
                                + production[j] / k0;
                        }
                    }
                }
                KineticsScheme::ImplicitEuler => {
                    // --- plain implicit Euler flux solve with the precursors
                    //     eliminated ---
                    let w_delayed: f64 = (0..n_dnp)
                        .map(|i| beta[i] * lambda[i] * dt / (1.0 + lambda[i] * dt))
                        .sum();
                    let time_derivative: Vec<f64> = inv_v.iter().map(|v| v / dt).collect();
                    let lhs = {
                        let with_diagonal = add_diagonal(&m_operator, &time_derivative)?;
                        linear_combination(&[
                            (1.0, &with_diagonal),
                            (-((1.0 - beta_total) + w_delayed) / k0, &sigma.f),
                        ])?
                    };
                    let mut rhs: Vec<f64> =
                        (0..philenf).map(|i| inv_v[i] * phi_old[i] / dt).collect();
                    for i in 0..n_dnp {
                        let weight = lambda[i] / (1.0 + lambda[i] * dt);
                        for j in 0..philenf {
                            rhs[j] += weight * precursors_old[i][j];
                        }
                    }
                    let lu = SparseLu::factorise(&lhs)?;
                    phi = lu.solve(&rhs);
                    fix_inf_nan(&mut phi);

                    // --- implicit Euler precursor update ---
                    let fs = spmv(&sigma.f, &phi);
                    for i in 0..n_dnp {
                        for j in 0..philenf {
                            precursors[i][j] = (precursors_old[i][j] + dt * beta[i] * fs[j] / k0)
                                / (1.0 + lambda[i] * dt);
                        }
                    }
                }
            }

            // --- transient T-H step ---
            let fp_phi = spmv(&sigma.fp, &phi);
            pwr = fp_phi
                .iter()
                .zip(vi_per_group.iter())
                .map(|(p, v)| p * v)
                .collect();
            relative_power_t = sum(&pwr) / power_0;
            th.power_ratio = power_ratio_0 * relative_power_t;
            th = seam::solve_thermal_hydraulics_transient(
                params,
                &geom_t,
                &th,
                &which_sigma_t,
                &pwr,
                &th_old,
                dt,
            );
        }

        // Lagged fission operator for the next step's F0 terms.
        sigma_f_old = sigma.f.clone();

        // --- record histories ---
        relative_power[n] = relative_power_t;
        avg_fuel_temp[n] = core_average_fuel_temperature(&th, &radial_weights, &fuel_volume);
        max_fuel_temp[n] = maximum_fuel_temperature(&th, &fuel_mask, radial_weights.len());
        coolant_outlet_temp[n] = average_coolant_outlet_temperature(&th, &outlet_index);

        pwr_node = collapse_power_over_groups(&grid, &pwr);
        if relative_power_t > relative_power_max {
            relative_power_max = relative_power_t;
            t_power_max = t;
            pwr_node_at_max = pwr_node.clone();
        }

        // Divergence guard: stop time stepping instead of marching a blown-up
        // solution to the end.
        // Written as three explicit tests to mirror the MATLAB
        // `~isfinite(prelt) || prelt<0 || prelt>1e12` line for line.
        #[allow(clippy::manual_range_contains)]
        let blown_up = !relative_power_t.is_finite()
            || relative_power_t < 0.0
            || relative_power_t > DIVERGENCE_POWER_RATIO;
        if blown_up {
            diverged = true;
            last_step = n + 1;
            break;
        }
    }

    let mut t_grid = t_grid;
    if diverged {
        n_t = last_step;
        t_grid.truncate(n_t);
        relative_power.truncate(n_t);
        avg_fuel_temp.truncate(n_t);
        max_fuel_temp.truncate(n_t);
        coolant_outlet_temp.truncate(n_t);
        rod_position.truncate(n_t);
    }

    let pwr_node_final = pwr_node;

    // ============================================================ //
    // ----- outputs (NEACRP-L-335 section 4 C) ----- //
    // ============================================================ //

    let z_scale = geometry.zscale;
    let mut radial_c5_z6 = radial_map_layer(&grid, &pwr_node_at_max, 6, z_scale);
    let mut radial_c5_z13 = radial_map_layer(&grid, &pwr_node_at_max, 13, z_scale);
    let mut radial_c6_z6 = radial_map_layer(&grid, &pwr_node_final, 6, z_scale);
    let mut radial_c6_z13 = radial_map_layer(&grid, &pwr_node_final, 13, z_scale);
    for map in [
        &mut radial_c5_z6,
        &mut radial_c5_z13,
        &mut radial_c6_z6,
        &mut radial_c6_z13,
    ] {
        normalise_to_peak(map);
    }

    Ok(TransientOutput {
        k_eff: k0,
        steady,
        th,
        time: t_grid,
        relative_power,
        avg_fuel_temp,
        max_fuel_temp,
        coolant_outlet_temp,
        radial_c5_z6,
        radial_c5_z13,
        radial_c6_z6,
        radial_c6_z13,
        t_power_max,
        relative_power_max,
        rod_position,
        scalar_flux_final: phi,
        pwrdens_final: pwr_node_final,
        precursors_final: precursors,
        time_scheme,
        diverged,
    })
}

/// `M = gradD + nodal + sigma.tot - sigma.s`, the static diffusion operator.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if the four terms disagree in shape.
pub fn diffusion_operator(
    grad_d: &SparseMatrix,
    nodal: &SparseMatrix,
    sigma_tot: &SparseMatrix,
    sigma_s: &SparseMatrix,
) -> Result<SparseMatrix> {
    linear_combination(&[
        (1.0, grad_d),
        (1.0, nodal),
        (1.0, sigma_tot),
        (-1.0, sigma_s),
    ])
}

/// Build the transient time grid \[s\].
///
/// MATLAB: `tgrid = [0 params.tgrid(:).' tend]`, then rounded to 1 µs and
/// deduplicated (`unique(round(tgrid*1e6))/1e6`) so overlapping range endpoints
/// cannot produce a near-zero step, then truncated at `tend`. With no case
/// grid, a uniform 10 ms grid over `0..tend` is used.
///
/// `unique` also **sorts**, so a case grid supplied out of order is silently
/// reordered — matching the MATLAB.
#[must_use]
pub fn build_time_grid(case_grid: Option<&[f64]>, t_end: f64) -> Vec<f64> {
    let mut raw: Vec<f64> = match case_grid {
        Some(g) => {
            let mut v = Vec::with_capacity(g.len() + 2);
            v.push(0.0);
            v.extend_from_slice(g);
            v.push(t_end);
            v
        }
        None => {
            let mut v = Vec::new();
            let mut t = 0.0_f64;
            let mut k = 0_usize;
            while t <= t_end {
                v.push(t);
                k += 1;
                t = 0.01 * k as f64;
            }
            v.push(t_end);
            v
        }
    };
    // round to 1 us, deduplicate, sort
    let mut micro: Vec<i64> = raw.drain(..).map(|t| (t * 1.0e6).round() as i64).collect();
    micro.sort_unstable();
    micro.dedup();
    let end_micro = (t_end * 1.0e6).round() as i64;
    micro
        .into_iter()
        .filter(|m| *m <= end_micro)
        .map(|m| m as f64 / 1.0e6)
        .collect()
}

/// `g0(x) = (exp(x) - 1 - x)/x²`, with the series fallback near `x = 0`.
///
/// MATLAB local function `gexp0`. The series `1/2 + x/6 + x²/24` is used for
/// `|x| < 1e-4`, where the direct form loses all its significant digits to
/// cancellation.
#[must_use]
pub fn g_exp_0(x: f64) -> f64 {
    if x.abs() < 1.0e-4 {
        0.5 + x / 6.0 + x * x / 24.0
    } else {
        (x.exp() - 1.0 - x) / (x * x)
    }
}

/// `g1(x) = (x - 1 + exp(-x))/x²`, with the series fallback near `x = 0`.
///
/// MATLAB local function `gexp1`. Series `1/2 - x/6 + x²/24` for `|x| < 1e-4`.
#[must_use]
pub fn g_exp_1(x: f64) -> f64 {
    if x.abs() < 1.0e-4 {
        0.5 - x / 6.0 + x * x / 24.0
    } else {
        (x - 1.0 + (-x).exp()) / (x * x)
    }
}

/// Per-node, per-group exponential-transform frequencies \[1/s\].
///
/// MATLAB local function `expfreq`, the Ants paper's Eq. (4):
/// `omega = ln(phi(t_n)/phi(t_{n-1}))/dt`. Zero wherever either flux is
/// non-positive or non-finite, or the node is void (`invv == 0`).
///
/// # Stability warning carried from the MATLAB
///
/// Yan Ren records this mode as **unstable in super-prompt rod ejections**:
/// node-wise frequency noise near the ejected channel feeds back through the
/// nearly singular prompt operator. [`global_group_frequencies`] is the
/// default for that reason.
#[must_use]
pub fn node_frequencies(phi_new: &[f64], phi_old: &[f64], dt: f64, inv_v: &[f64]) -> Vec<f64> {
    (0..phi_new.len())
        .map(|i| {
            let ok = phi_new[i].is_finite()
                && phi_old[i].is_finite()
                && phi_new[i] > 0.0
                && phi_old[i] > 0.0
                && inv_v[i] > 0.0;
            if ok {
                (phi_new[i] / phi_old[i]).ln() / dt
            } else {
                0.0
            }
        })
        .collect()
}

/// Per-group **global** amplitude frequencies \[1/s\], uniform in space.
///
/// The default mode. Taken from the volume-integrated group flux, so it
/// captures the stiff exponential amplitude rise of a super-prompt excursion
/// exactly while carrying no spatial noise. Zero on void nodes (`invv == 0`),
/// and left at zero for any group whose integrated flux is non-positive or
/// non-finite at either end of the step.
#[must_use]
pub fn global_group_frequencies(
    grid: &crate::reference::grid::Grid,
    philenf: usize,
    phi: &[f64],
    phi_old: &[f64],
    vi_per_group: &[f64],
    inv_v: &[f64],
    dt: f64,
) -> Vec<f64> {
    let nodes = grid.nodes();
    let mut omega = vec![0.0_f64; philenf];
    for g in 0..grid.ngroups {
        let range = g * nodes..(g + 1) * nodes;
        let mut numerator = 0.0_f64;
        let mut denominator = 0.0_f64;
        for i in range.clone() {
            let mask = f64::from(inv_v[i] > 0.0);
            numerator += phi[i] * vi_per_group[i] * mask;
            denominator += phi_old[i] * vi_per_group[i] * mask;
        }
        if numerator.is_finite() && denominator.is_finite() && numerator > 0.0 && denominator > 0.0
        {
            let value = (numerator / denominator).ln() / dt;
            for i in range {
                omega[i] = value * f64::from(inv_v[i] > 0.0);
            }
        }
    }
    omega
}

/// Fuel-node mask: 1 where the node is fuel, 0 elsewhere.
///
/// MATLAB: `whichsigmaref(ix,iy,iz) >= 4`, with the comment *"compositions 4-11
/// are fuel in the NEACRP composition map"*.
///
/// # Case-specific constant
///
/// The threshold 4 is **hard-coded to the NEACRP composition numbering** and is
/// not derived from the case data. Any case whose fuel compositions are not
/// numbered 4 and above gets a wrong mask, and hence wrong C2/C3/C4 outputs,
/// silently. Recorded, not fixed.
#[must_use]
pub fn fuel_node_mask(
    grid: &crate::reference::grid::Grid,
    which_sigma_ref: &MaterialMap,
) -> Vec<f64> {
    let mut mask = vec![0.0_f64; grid.nodes()];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                if which_sigma_ref.at(ix, iy, iz) >= 4 {
                    mask[grid.index(0, ix, iy, iz)] = 1.0;
                }
            }
        }
    }
    mask
}

/// Spatial indices of every fuel-bearing channel's outlet node.
///
/// MATLAB: the top node of every column that contains any fuel, taken as
/// `geometry.zhis(ix,iy)` — the highest fuel-bearing axial node, **not** the
/// top of the mesh.
#[must_use]
pub fn channel_outlet_indices(
    grid: &crate::reference::grid::Grid,
    fuel_mask: &[f64],
    zhis: &[usize],
) -> Vec<usize> {
    let mut out = Vec::new();
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let base = grid.index(0, ix, iy, 0);
            let has_fuel = (0..grid.nz).any(|iz| fuel_mask[base + iz] != 0.0);
            if has_fuel {
                // MATLAB `col + geometry.zhis(ix,iy)` with a 1-based zhis.
                out.push(base + zhis[ix * grid.ny + iy] - 1);
            }
        }
    }
    out
}

/// Radial volume weights for the in-rod fuel-temperature average \[-\].
///
/// MATLAB: solution ids `1..fueln` are the fuel rings up to their centres and
/// id `fueln+1` is the fuel surface node covering `[Ctr(fueln), fuelrad]`, so
/// the weights are annular areas normalised by the pellet area:
///
/// ```text
/// w(1)       = Ctr(1)^2 / R^2
/// w(i)       = (Ctr(i)^2 - Ctr(i-1)^2) / R^2      for 2 <= i <= fueln
/// w(fueln+1) = (R^2 - Ctr(fueln)^2) / R^2
/// ```
///
/// Returns `fueln + 1` weights.
#[must_use]
pub fn fuel_radial_weights(params: &CaseParams, geometry: &CoreGeometry) -> Vec<f64> {
    let fuel_n = params.fuel_n;
    let ctr = &geometry.fuel.ctr;
    let r = geometry.fuel.fuel_rad;
    let mut w = vec![0.0_f64; fuel_n + 1];
    w[0] = ctr[0] * ctr[0];
    for i in 1..fuel_n {
        w[i] = ctr[i] * ctr[i] - ctr[i - 1] * ctr[i - 1];
    }
    w[fuel_n] = r * r - ctr[fuel_n - 1] * ctr[fuel_n - 1];
    for v in &mut w {
        *v /= r * r;
    }
    w
}

/// Fuel-volume-weighted core-average fuel temperature \[K\] — output **C2**.
///
/// MATLAB `calcavgfuel`: the radial average over the pellet
/// (`fueltemp(:,1:fueln+1) * wrad`) weighted by the fuel-node volume.
///
/// # Panics
///
/// If `th.fuel_temp` is shorter than `nodes * radial_weights.len()`.
#[must_use]
pub fn core_average_fuel_temperature(
    th: &ThermalState,
    radial_weights: &[f64],
    fuel_volume: &[f64],
) -> f64 {
    let ids = th.n_solution_ids;
    let mut numerator = 0.0_f64;
    for (node, &volume) in fuel_volume.iter().enumerate() {
        let mut radial_average = 0.0_f64;
        for (i, &w) in radial_weights.iter().enumerate() {
            radial_average += th.fuel_temp[node * ids + i] * w;
        }
        numerator += radial_average * volume;
    }
    numerator / fuel_volume.iter().sum::<f64>()
}

/// Maximum fuel temperature over the pellet of any fuel node \[K\] — output
/// **C3**.
///
/// MATLAB `calcmaxfuel`: `max(max(fueltemp(fuelmask==1, 1:fueln+1)))`.
#[must_use]
pub fn maximum_fuel_temperature(th: &ThermalState, fuel_mask: &[f64], n_radial: usize) -> f64 {
    let ids = th.n_solution_ids;
    let mut peak = f64::NEG_INFINITY;
    for (node, &m) in fuel_mask.iter().enumerate() {
        if m != 1.0 {
            continue;
        }
        for i in 0..n_radial {
            peak = peak.max(th.fuel_temp[node * ids + i]);
        }
    }
    peak
}

/// Mean coolant temperature over the channel outlet nodes \[K\] — output
/// **C4**.
///
/// MATLAB `calccoolout`: `mean(th.coolant.temps(outletidx))`. An unweighted
/// mean over channels, not a flow-weighted mixed-mean outlet temperature.
#[must_use]
pub fn average_coolant_outlet_temperature(th: &ThermalState, outlet_index: &[usize]) -> f64 {
    if outlet_index.is_empty() {
        return f64::NAN;
    }
    let total: f64 = outlet_index.iter().map(|&i| th.coolant.temps[i]).sum();
    total / outlet_index.len() as f64
}

/// Sum a state-vector power over energy groups, leaving one value per node.
///
/// MATLAB `collapsepow = @(pwr) sum(reshape(pwr, es, G), 2)`.
#[must_use]
pub fn collapse_power_over_groups(grid: &crate::reference::grid::Grid, power: &[f64]) -> Vec<f64> {
    let nodes = grid.nodes();
    let mut out = vec![0.0_f64; nodes];
    for g in 0..grid.ngroups {
        for node in 0..nodes {
            out[node] += power[g * nodes + node];
        }
    }
    out
}

/// Radial (x-y) power map of an **active-core** axial layer, `nx*ny` entries
/// indexed `ix*ny + iy`.
///
/// MATLAB local function `radialmaplayer`. The axial blocks of the NEACRP model
/// are: block 1 the lower reflector, blocks 2–17 the active core, block 18 the
/// upper reflector, each spanning `zscale` mesh layers. Active layer `L`
/// therefore spans global mesh layers `L*zscale+1 … (L+1)*zscale`, which is why
/// the offset is `L*zscale` and not `(L-1)*zscale`.
///
/// # Panics
///
/// If `layer` and `z_scale` address a mesh layer beyond `grid.nz` — which the
/// MATLAB would also do, as an index error.
#[must_use]
pub fn radial_map_layer(
    grid: &crate::reference::grid::Grid,
    node_power: &[f64],
    layer: usize,
    z_scale: usize,
) -> Vec<f64> {
    let mut map = vec![0.0_f64; grid.nx * grid.ny];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let base = grid.index(0, ix, iy, 0);
            let mut total = 0.0_f64;
            for k in 0..z_scale {
                let iz = layer * z_scale + k;
                total += node_power[base + iz];
            }
            map[ix * grid.ny + iy] = total;
        }
    }
    map
}

/// Scale a map so its peak is 1 — the MATLAB `radC5_z6/max(radC5_z6(:))`.
///
/// A non-positive or non-finite peak leaves the map untouched rather than
/// filling it with NaN; MATLAB would divide anyway. Recorded as a deliberate
/// difference in a degenerate case that produces no meaningful output either
/// way.
pub fn normalise_to_peak(map: &mut [f64]) {
    let peak = map.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if peak.is_finite() && peak > 0.0 {
        for v in map.iter_mut() {
            *v /= peak;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;

    #[test]
    fn series_fallbacks_match_the_direct_forms_at_the_switch_point() {
        // Just outside the |x| < 1e-4 window, the direct form is still accurate,
        // so the two must agree there to many digits.
        let x = 1.0e-3_f64;
        let direct_0 = (x.exp() - 1.0 - x) / (x * x);
        assert!((g_exp_0(x) - direct_0).abs() < 1e-12);
        let direct_1 = (x - 1.0 + (-x).exp()) / (x * x);
        assert!((g_exp_1(x) - direct_1).abs() < 1e-12);
    }

    #[test]
    fn series_fallbacks_tend_to_one_half_at_zero() {
        assert!((g_exp_0(0.0) - 0.5).abs() < 1e-15);
        assert!((g_exp_1(0.0) - 0.5).abs() < 1e-15);
    }

    #[test]
    fn time_grid_deduplicates_overlapping_range_endpoints() {
        // The NEACRP D1 pattern: consecutive ranges share their endpoints.
        let case_grid = [0.0, 0.5, 1.0, 1.0, 1.5, 2.0];
        let grid = build_time_grid(Some(&case_grid), 2.0);
        assert_eq!(grid, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
        // No zero-length step survives.
        for w in grid.windows(2) {
            assert!(w[1] - w[0] > 0.0);
        }
    }

    #[test]
    fn time_grid_truncates_above_the_end_time() {
        let grid = build_time_grid(Some(&[0.0, 1.0, 5.0]), 2.0);
        assert_eq!(grid, vec![0.0, 1.0, 2.0]);
    }

    #[test]
    fn default_time_grid_is_uniform_ten_milliseconds() {
        let grid = build_time_grid(None, 0.05);
        assert_eq!(grid, vec![0.0, 0.01, 0.02, 0.03, 0.04, 0.05]);
    }

    #[test]
    fn node_frequencies_are_zero_on_void_and_non_positive_flux() {
        let phi = [std::f64::consts::E, 1.0, -1.0];
        let phi_old = [1.0, 1.0, 1.0];
        let inv_v = [1.0, 0.0, 1.0];
        let omega = node_frequencies(&phi, &phi_old, 1.0, &inv_v);
        assert!((omega[0] - 1.0).abs() < 1e-12); // ln(e)/1
        assert_eq!(omega[1], 0.0); // void node
        assert_eq!(omega[2], 0.0); // negative flux
    }

    #[test]
    fn group_collapse_sums_over_groups_only() {
        let grid = Grid::new(2, 1, 1, 2).expect("valid grid");
        // group 0: [1, 2]; group 1: [10, 20]
        let power = [1.0, 2.0, 10.0, 20.0];
        assert_eq!(collapse_power_over_groups(&grid, &power), vec![11.0, 22.0]);
    }

    #[test]
    fn radial_map_offsets_past_the_lower_reflector() {
        // nz = 6, zscale = 2: active layer 1 spans mesh layers 2 and 3 (0-based).
        let grid = Grid::new(1, 1, 6, 1).expect("valid grid");
        let power = [0.0, 0.0, 3.0, 4.0, 0.0, 0.0];
        assert_eq!(radial_map_layer(&grid, &power, 1, 2), vec![7.0]);
    }

    #[test]
    fn peak_normalisation_leaves_a_degenerate_map_alone() {
        let mut map = [0.0, 0.0];
        normalise_to_peak(&mut map);
        assert_eq!(map, [0.0, 0.0]);
        let mut map = [1.0, 2.0];
        normalise_to_peak(&mut map);
        assert_eq!(map, [0.5, 1.0]);
    }
}
