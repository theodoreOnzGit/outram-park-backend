//! Steady coupled neutronics/thermal-hydraulics solve.
//!
//! # Provenance
//!
//! Translated from `thdiffusion_solverxyz.m` in Than Yan Ren's (SNRSI) BEDOK
//! MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received
//! 2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear Research
//! and Safety Institute. Translated with permission; see
//! `docs/bedok-port-scoping.md` §6.

use super::cross_section_feedback::update_cross_sections;
use super::error::Result;
use super::seam::{
    self, CaseParams, CoreGeometry, DiffusionSolution, MaterialMap, SigmaValues, ThermalState,
};
use super::sparse::{max_abs_difference, norm2, spmv, sum, under_relax};

/// Default fuel-temperature convergence tolerance \[K\].
///
/// Yan Ren's note, kept verbatim because it is a physics judgement and not a
/// tuning knob: *"relaxed from 0.01 K; a max-norm fuel temperature criterion
/// that tight is unrealistic for a coupled BWR steady state — the hot nodes
/// limit-cycle ~1 K"*.
pub const DEFAULT_FUEL_TEMP_TOL: f64 = 0.5;

/// Default outer fission-source / `k_eff` tolerance \[-\].
///
/// Yan Ren's note: *"Relaxed from 1e-5: even exact inner solves floor the
/// outer fission-source residual near ~1e-4 (a tiny residual Picard cycle), so
/// 1e-5 is unreachable."*
pub const DEFAULT_FLUX_TOL: f64 = 1.0e-4;

/// Default cap on coupled outer iterations.
pub const DEFAULT_MAX_OUTER_ITERATIONS: usize = 50;

/// Default Picard under-relaxation factor for the T-H feedback fields \[-\].
pub const DEFAULT_TH_RELAX: f64 = 0.5;

/// Floor of the inexact inner-solve tolerance schedule \[-\].
pub const INNER_TOL_FLOOR: f64 = 1.0e-6;

/// Cap of the inexact inner-solve tolerance schedule \[-\].
pub const INNER_TOL_CAP: f64 = 1.0e-3;

/// Default forcing factor of the inexact inner-solve schedule \[-\].
pub const DEFAULT_INEXACT_ETA: f64 = 0.001;

/// Result of a steady coupled solve — the MATLAB `output` struct of
/// `thdiffusion_solverxyz.m`.
#[derive(Debug, Clone)]
pub struct SteadyOutput {
    /// Converged multiplication factor \[-\].
    pub k_eff: f64,
    /// Final fission-source residual \[-\], `‖Δfs‖₂/‖fs‖₂`.
    pub residual: f64,
    /// Final `k_eff` residual \[-\], `|Δk|/k`.
    pub k_eff_residual: f64,
    /// Final fuel-temperature change \[K\], `max|Δ T_fuel,avg|`.
    pub fuel_temp_residual: f64,
    /// Per-iteration fuel-temperature change history \[K\]. Entries for
    /// iterations where no T-H update ran are [`f64::INFINITY`], as in the
    /// MATLAB's `inf(maxiter,1)` preallocation.
    pub fuel_temp_residual_history: Vec<f64>,
    /// Per-iteration `k_eff` history \[-\], starting with the initial guess.
    pub k_eff_history: Vec<f64>,
    /// Converged scalar flux, renormalised so the fission-source integral
    /// matches its initial value. `state_len` entries.
    pub scalar_flux: Vec<f64>,
    /// Fission source at the same normalisation, `state_len` entries.
    pub fission_source: Vec<f64>,
    /// Node power density, `fission_source .* Vi`, `state_len` entries.
    pub pwrdens: Vec<f64>,
    /// Converged thermal-hydraulic state.
    pub th: ThermalState,
    /// `false` if the loop bailed out through the not-converging guard —
    /// `k_eff` non-positive or NaN, or the outer-iteration cap passed.
    ///
    /// Has no MATLAB counterpart: the original prints
    /// `" T-H interation stopped, not converging"` and returns the same struct
    /// either way, so a caller cannot tell. Recorded here because
    /// [`critical_boron`](super::critical_boron) has to re-derive it from the
    /// returned `k_eff`.
    pub converged: bool,
}

/// Number of radial solution ids in the fuel rod — MATLAB `maxid`.
///
/// `maxir` rings plus one extra node at every material interface, found by
/// counting transitions to and from `whichk == 0` (the gap) along the radius.
///
/// # Panics
///
/// If `which_k` is shorter than `fuel_max_ir`.
#[must_use]
pub fn fuel_solution_id_count(params: &CaseParams, geometry: &CoreGeometry) -> usize {
    let which_k = &geometry.fuel.which_k;
    let mut surface_count = 0_usize;
    for ir in 0..params.fuel_max_ir - 1 {
        let here = which_k[ir];
        let next = which_k[ir + 1];
        if (here != 0 && next == 0) || (here == 0 && next != 0) {
            surface_count += 1;
        }
    }
    params.fuel_max_ir + surface_count
}

/// Flat starting thermal-hydraulic state — the MATLAB "Set up initial T-H"
/// block.
///
/// Every node starts at the case's average fuel temperature, average coolant
/// temperature and average coolant density, with zero wall heat flux. The
/// non-field members of `th` (flow rate, rated power, pin count) are carried
/// through untouched.
#[must_use]
pub fn flat_initial_thermal_state(
    params: &CaseParams,
    geometry: &CoreGeometry,
    th: &ThermalState,
) -> ThermalState {
    let nodes = params.grid.nodes();
    let n_solution_ids = fuel_solution_id_count(params, geometry);
    let mut initial = th.clone();
    initial.fuel_temp_avg = vec![params.fuel_temp_avg_init; nodes];
    initial.fuel_temp_doppler = vec![params.fuel_temp_avg_init; nodes];
    initial.fuel_temp = vec![params.fuel_temp_avg_init; nodes * n_solution_ids];
    initial.n_solution_ids = n_solution_ids;
    initial.coolant.temps = vec![params.cool_temp_avg_init; nodes];
    initial.coolant.dens = vec![params.cool_den_avg_init; nodes];
    initial.heat_flux = vec![0.0; nodes];
    initial
}

/// Solve the coupled steady state.
///
/// MATLAB `thdiffusion_solverxyz(geometry, params, th, sigmavalues,
/// whichsigma, initial_k_eff)`.
///
/// # The iteration
///
/// One Picard cycle per outer iteration:
///
/// 1. Update the cross sections at the current T-H state
///    ([`update_cross_sections`]) — skipped on the first pass, which uses the
///    update already done before the loop.
/// 2. Pick an inexact inner tolerance from how far the outer loop still is
///    (see [`inexact_inner_tolerance`]).
/// 3. Solve the `k`-eigenvalue problem, **warm-started** from the previous
///    outer iteration's flux and `k_eff`.
/// 4. Measure the fission-source and `k_eff` residuals.
/// 5. Take one steady T-H step and **under-relax** the four fields that carry
///    the feedback: coolant density, Doppler temperature, average fuel
///    temperature and wall heat flux.
///
/// The loop exits when all three of the fission-source residual, the `k_eff`
/// residual and the fuel-temperature change are below tolerance; or bails out
/// if `k_eff` goes non-positive or NaN, or the iteration cap is passed.
///
/// # Arguments
///
/// - `initial_k_eff` — MATLAB `varargin{1}`; 1.0 when the caller passes
///   [`None`].
///
/// # Normalisation
///
/// On exit the flux and fission source are scaled so the fission-source
/// integral equals its value at the *initial flat flux*
/// (`init_norm = sum(sigma_f * ones)`), the MATLAB's
/// "fission source integration = 1" convention.
///
/// # Deviation — file output
///
/// The MATLAB ends with seven `writematrix` calls that dump `k_eff`, the
/// residual histories, the flux, the fission source and the power density into
/// the working directory unconditionally. A library function writing into a
/// caller's working directory is not acceptable here, so the same data is
/// returned in [`SteadyOutput`] instead and no file is written. Numerically
/// nothing changes.
///
/// # Deviation — progress printing
///
/// The MATLAB's per-iteration `fprintf` lines are not reproduced; the same
/// numbers are in [`SteadyOutput::k_eff_history`],
/// [`SteadyOutput::fuel_temp_residual_history`] and the residual fields.
///
/// # Errors
///
/// Propagates the cross-section feedback's `pauseonnan` guards and any sparse
/// failure.
///
/// # Panics
///
/// Through the [`seam`] stubs until `nodal/` and `th/` land.
pub fn solve_coupled_steady(
    geometry: &CoreGeometry,
    params: &CaseParams,
    th: &ThermalState,
    sigma_values: &SigmaValues,
    which_sigma: &MaterialMap,
    initial_k_eff: Option<f64>,
) -> Result<SteadyOutput> {
    // MATLAB mutates its local `params` (params.innertol); so do we.
    let mut params = params.clone();
    let grid = params.grid;
    let initial_k_eff = initial_k_eff.unwrap_or(1.0);

    let vi_per_group = seam::replicate_per_group(&grid, &geometry.base.volume);

    let fuel_temp_tol = params.fuel_temp_tol.unwrap_or(DEFAULT_FUEL_TEMP_TOL);
    let flux_tol = params.flux_tol.unwrap_or(DEFAULT_FLUX_TOL);
    let max_iter = params.th_max_iter.unwrap_or(DEFAULT_MAX_OUTER_ITERATIONS);
    let w_relax = params.th_relax.unwrap_or(DEFAULT_TH_RELAX);

    let sigma_values_ref = sigma_values;
    let which_sigma_ref = which_sigma;

    // ----- Set up initial T-H ----- //
    let mut th = flat_initial_thermal_state(&params, geometry, th);

    // ----- Set up initial values ----- //
    let mut scalar_flux = vec![1.0_f64; grid.state_len()];

    // MATLAB preallocates these to `maxiter` and grows them by assignment; the
    // fill values differ per array (`ones`, `ones`, `inf`, `zeros`) and the
    // grown tail is always zero, which the helpers below reproduce.
    let mut residual = vec![1.0_f64; max_iter];
    let mut k_eff_residual = vec![1.0_f64; max_iter];
    let mut fuel_temp_residual_history = vec![f64::INFINITY; max_iter];
    let mut k_eff = vec![0.0_f64; max_iter];
    k_eff[0] = initial_k_eff;

    // MATLAB `iteration` is 1-based; kept 1-based here so the array indexing
    // below reads the same as the original, with `-1` only at the access.
    let mut iteration = 1_usize;

    // ----- update sigmavalues ----- //
    let (mut sigma_values_now, mut which_sigma_now) =
        update_cross_sections(&params, geometry, sigma_values_ref, which_sigma_ref, &th)?;
    let sigma = seam::make_sigma_operators(&params, &sigma_values_now, &which_sigma_now);

    let mut fission_source = spmv(&sigma.f, &scalar_flux);
    let init_norm = sum(&fission_source);
    drop(sigma); // MATLAB `clear('sigma')`

    let mut fuel_temp_avg = th.fuel_temp_avg.clone();
    let mut fuel_temp_error = f64::INFINITY;

    let mut fission_source_new = fission_source.clone();
    let mut converged = true;

    // ----- Run source iteration ----- //
    while at(&residual, iteration) >= flux_tol
        || at(&k_eff_residual, iteration) >= flux_tol
        || fuel_temp_error >= fuel_temp_tol
    {
        if iteration > 1 {
            let (v, w) =
                update_cross_sections(&params, geometry, sigma_values_ref, which_sigma_ref, &th)?;
            sigma_values_now = v;
            which_sigma_now = w;
        }

        params.inner_tol = inexact_inner_tolerance(
            &params,
            at(&residual, iteration),
            at(&k_eff_residual, iteration),
        );

        // Warm-started inner eigenvalue solve.
        let diffresults: DiffusionSolution = seam::solve_sanodal_eigenvalue(
            geometry,
            &params,
            &sigma_values_now,
            &which_sigma_now,
            at(&k_eff, iteration),
            Some(&scalar_flux),
        );
        let scalar_flux_l_plus = diffresults.scalar_flux.clone();
        set_at(&mut k_eff, iteration + 1, diffresults.k_eff);
        fission_source_new = diffresults.fission_source.clone();

        let difference: Vec<f64> = fission_source_new
            .iter()
            .zip(fission_source.iter())
            .map(|(a, b)| a - b)
            .collect();
        set_at(
            &mut residual,
            iteration + 1,
            norm2(&difference) / norm2(&fission_source),
        );

        set_at(
            &mut k_eff_residual,
            iteration + 1,
            (at(&k_eff, iteration + 1) - at(&k_eff, iteration)).abs() / at(&k_eff, iteration),
        );

        // Stop iteration if not-converging.
        let next_k = at(&k_eff, iteration + 1);
        if next_k <= 0.0 || next_k.is_nan() || iteration > max_iter {
            converged = false;
            iteration += 1;
            break;
        }

        // Update T-H, with Picard under-relaxation of the feedback fields.
        let th_old = th.clone();
        th = seam::solve_thermal_hydraulics_steady(
            &params,
            geometry,
            &th,
            &which_sigma_now,
            &diffresults.pwrdens,
        );
        under_relax(&mut th.coolant.dens, &th_old.coolant.dens, w_relax);
        under_relax(
            &mut th.fuel_temp_doppler,
            &th_old.fuel_temp_doppler,
            w_relax,
        );
        under_relax(&mut th.fuel_temp_avg, &th_old.fuel_temp_avg, w_relax);
        under_relax(&mut th.heat_flux, &th_old.heat_flux, w_relax);

        let fuel_temp_avg_new = th.fuel_temp_avg.clone();
        // `abs`: a falling temperature must still count as error.
        fuel_temp_error = max_abs_difference(&fuel_temp_avg_new, &fuel_temp_avg);
        set_at(
            &mut fuel_temp_residual_history,
            iteration + 1,
            fuel_temp_error,
        );

        iteration += 1;

        scalar_flux = scalar_flux_l_plus;
        fission_source = fission_source_new.clone();
        fuel_temp_avg = fuel_temp_avg_new;
    }

    // CURRENT NORMALIZATION: fission source integration = 1
    let norm_factor = sum(&fission_source_new);
    let scale = init_norm / norm_factor;
    for v in &mut scalar_flux {
        *v *= scale;
    }
    for v in &mut fission_source {
        *v *= scale;
    }

    // ------- do CHF ---------- //
    // Faithful: the MATLAB computes this and never uses it — `chf` does not
    // appear in the output struct. Kept so the call site is visible when the
    // dead result is eventually put to use.
    let _chf = seam::w3_chf_hottest_channel(&params, geometry, &th);

    // ----- output ----- //
    // Trim the per-iteration histories to the iterations actually performed.
    let n_last = iteration.min(k_eff.len());
    k_eff.truncate(n_last);
    residual.truncate(n_last);
    k_eff_residual.truncate(n_last);
    fuel_temp_residual_history.truncate(n_last.min(fuel_temp_residual_history.len()));
    let ft_last = fuel_temp_residual_history
        .iter()
        .rposition(|v| v.is_finite())
        .unwrap_or(fuel_temp_residual_history.len().saturating_sub(1));
    let ft_residual = fuel_temp_residual_history
        .get(ft_last)
        .copied()
        .unwrap_or(f64::INFINITY);

    let pwrdens: Vec<f64> = fission_source
        .iter()
        .zip(vi_per_group.iter())
        .map(|(f, v)| f * v)
        .collect();

    Ok(SteadyOutput {
        k_eff: k_eff[n_last - 1],
        residual: residual[n_last - 1],
        k_eff_residual: k_eff_residual[n_last - 1],
        fuel_temp_residual: ft_residual,
        fuel_temp_residual_history,
        k_eff_history: k_eff,
        scalar_flux,
        fission_source,
        pwrdens,
        th,
        converged,
    })
}

/// Inexact inner-solve tolerance for the next eigenvalue solve \[-\].
///
/// MATLAB `thdiffusion_solverxyz.m:118-134`, an Eisenstat-Walker-style forcing
/// schedule. While the outer neutronics/T-H loop is far from converged, an
/// over-tight inner solve is wasted work because the cross sections change
/// again next pass; so
///
/// ```text
/// innertol = clamp(eta * max(fission-source residual, k_eff residual),
///                  1e-6, 1e-3)
/// ```
///
/// Yan Ren's reasoning, which is a physics point rather than a performance
/// one: *"A loose inner solve does not merely blur the final keff readout — it
/// biases the coupled FIXED POINT through the feedback (loose flux → wrong
/// power → wrong fuel temp → wrong Doppler)."* `eta = 0.001` makes the schedule
/// self-tighten to the 1e-6 floor in the tail, killing the power-shape jitter
/// and the fuel-temperature limit cycle it caused.
///
/// Returns [`None`] — leaving the inner solver at its own 1e-6 default — when
/// `params.inexactinner == 0`.
#[must_use]
pub fn inexact_inner_tolerance(
    params: &CaseParams,
    fission_source_residual: f64,
    k_eff_residual: f64,
) -> Option<f64> {
    if params.inexact_inner == Some(0.0) {
        return params.inner_tol;
    }
    let eta = params.inexact_eta.unwrap_or(DEFAULT_INEXACT_ETA);
    let outer_residual = fission_source_residual.max(k_eff_residual);
    Some(INNER_TOL_CAP.min(INNER_TOL_FLOOR.max(eta * outer_residual)))
}

/// Read a MATLAB 1-based history entry, treating a past-the-end read as the
/// MATLAB growth fill of zero.
pub(super) fn at(v: &[f64], one_based: usize) -> f64 {
    v.get(one_based - 1).copied().unwrap_or(0.0)
}

/// Write a MATLAB 1-based history entry, growing the vector with zeros exactly
/// as MATLAB grows a numeric array on out-of-range assignment.
pub(super) fn set_at(v: &mut Vec<f64>, one_based: usize, value: f64) {
    if one_based > v.len() {
        v.resize(one_based, 0.0);
    }
    v[one_based - 1] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matlab_style_growth_pads_with_zeros() {
        let mut v = vec![1.0, 1.0];
        set_at(&mut v, 5, 7.0);
        assert_eq!(v, vec![1.0, 1.0, 0.0, 0.0, 7.0]);
        assert_eq!(at(&v, 3), 0.0);
        assert_eq!(at(&v, 99), 0.0);
    }

    #[test]
    fn inner_tolerance_clamps_to_the_documented_band() {
        let mut params = super::super::tests_support::minimal_params();
        params.inexact_eta = Some(0.001);
        // A large outer residual is capped at 1e-3.
        assert_eq!(inexact_inner_tolerance(&params, 10.0, 0.0), Some(1.0e-3));
        // A tiny one is floored at 1e-6.
        assert_eq!(inexact_inner_tolerance(&params, 1e-12, 0.0), Some(1.0e-6));
        // In between it is eta * the larger residual.
        assert_eq!(inexact_inner_tolerance(&params, 1e-3, 1e-4), Some(1.0e-6));
        assert_eq!(inexact_inner_tolerance(&params, 0.1, 0.0), Some(1.0e-4));
    }

    #[test]
    fn disabling_the_schedule_leaves_the_inner_tolerance_alone() {
        let mut params = super::super::tests_support::minimal_params();
        params.inexact_inner = Some(0.0);
        params.inner_tol = Some(1.0e-8);
        assert_eq!(inexact_inner_tolerance(&params, 1.0, 1.0), Some(1.0e-8));
    }

    #[test]
    fn solution_id_count_adds_one_per_material_interface() {
        let params = super::super::tests_support::minimal_params();
        // whichk = [1 1 0 2 2]: one fuel->gap transition and one gap->clad
        // transition, so two extra surface nodes.
        let geometry = super::super::tests_support::minimal_geometry_with_which_k(
            &params,
            vec![1, 1, 0, 2, 2],
        );
        assert_eq!(fuel_solution_id_count(&params, &geometry), 5 + 2);
    }
}
