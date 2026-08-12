//! Critical boron concentration search for the coupled steady state.
//!
//! # Provenance
//!
//! Translated from `criticalboron_xyz.m` in Than Yan Ren's (SNRSI) BEDOK
//! MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received
//! 2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear Research
//! and Safety Institute. Translated with permission; see
//! `docs/bedok-port-scoping.md` §6.
//!
//! # Why the search is built this way
//!
//! Yan Ren's own note on the 2026-06 rewrite, kept because it explains a design
//! that otherwise looks over-elaborate: the previous implementation wrapped a
//! secant iteration around full **cold-started** coupled solves, one per boron
//! iterate. That cold-start T-H Picard *"can go chaotic at off-nominal boron
//! (keff transients into the hundreds)"* and either trips the solver's
//! not-converging exit — returning a garbage `k_eff` that poisons the secant,
//! with boron observed diverging past 1e5 ppm — or settles into a spurious
//! coupled state.
//!
//! The rewrite never cold-starts the T-H away from the starting boron:
//!
//! - **Phase 0** — one coupled steady solve at the starting boron, with a
//!   robust bootstrap ([`eigensolve_cold`]) if the standard solver diverges
//!   from its cold start.
//! - **Phase 1** — a guarded secant on *static* eigenvalue solves at the frozen
//!   Phase-0 T-H state. Cheap, and it measures the boron worth slope.
//! - **Phase 2** — a warm-started coupled loop: per outer iteration one static
//!   eigensolve, a boron correction using the measured slope, and one
//!   under-relaxed static T-H update, converging boron, flux and feedback
//!   together.
//!
//! # Two eigensolvers, and why both are needed
//!
//! [`eigensolve_at_boron`] delegates to the production SA-nodal eigensolver
//! warm-started from the running flux. [`eigensolve_cold`] instead builds the
//! nodal correction from the incoming flux, **freezes** it, and runs a
//! self-normalising power iteration. Yan Ren verified both halves of the
//! reason:
//!
//! - the production solver's *continuous* nodal updates use the still-bad
//!   mid-iteration flux on a cold start and diverge (`k_eff → 5e4`) on a
//!   heavily rodded configuration; freezing them via `params.nodalupd` does
//!   stabilise it, **but**
//! - the production solver builds its *initial* nodal correction from a
//!   hardcoded flat flux, so a frozen call returns a ~25 pcm-biased, flatter
//!   seed — which then destabilised a near-critical Phase-1 warm solve
//!   (`k_eff → 377`).
//!
//! [`eigensolve_cold`] is stable cold *and* returns an accurate seed, which the
//! production solver cannot be made to do through its parameters.

use super::cross_section_feedback::update_cross_sections;
use super::error::{CouplingError, Result};
use super::seam::{self, CaseParams, CoreGeometry, MaterialMap, NodalTerms, SigmaValues, ThermalState};
use super::sparse::{fix_inf_nan, max_abs_difference, norm1, norm2, spmv, sum, under_relax, SparseLu};
use super::steady::{
    fuel_solution_id_count, set_at, solve_coupled_steady, DEFAULT_FUEL_TEMP_TOL, DEFAULT_TH_RELAX,
};
use super::transient::diffusion_operator;

/// Default `|k_eff - 1|` tolerance of the critical state \[-\].
pub const DEFAULT_CRIT_TOL: f64 = 1.0e-5;

/// Secant seed for the boron worth, `dk/db` \[1/ppm\] — a typical PWR value.
pub const DEFAULT_BORON_WORTH_SLOPE: f64 = -9.0e-5;

/// Phase-2 outer-iteration cap.
pub const MAX_COUPLED_ITERATIONS: usize = 40;

/// Phase-1 secant-iteration cap.
pub const MAX_SECANT_ITERATIONS: usize = 12;

/// Phase-1 secant convergence tolerance on `|k_eff - 1|` \[-\].
///
/// Tighter than [`DEFAULT_CRIT_TOL`] because the frozen-T-H eigensolves are
/// cheap and the slope measurement wants a clean bracket.
pub const SECANT_TOL: f64 = 2.0e-6;

/// Sane-range guard on `k_eff` in Phases 1 and 2 \[-\].
pub const SANE_K_EFF: (f64, f64) = (0.8, 1.2);

/// Sane-range guard on `k_eff` during the Phase-0 bootstrap \[-\], deliberately
/// wider than [`SANE_K_EFF`].
pub const SANE_K_EFF_BOOTSTRAP: (f64, f64) = (0.5, 1.5);

/// Phase-0 bootstrap iteration cap.
pub const MAX_BOOTSTRAP_ITERATIONS: usize = 30;

/// Inner tolerance forced on the Phase-1/2 eigensolves \[-\].
///
/// Tight, for a sub-ppm-accurate critical `k_eff`.
pub const SEARCH_INNER_TOL: f64 = 1.0e-8;

/// Nodal-correction refinements in [`eigensolve_cold`].
pub const COLD_NODAL_REFINEMENTS: usize = 3;

/// Power-iteration cap in [`eigensolve_cold`].
pub const COLD_MAX_POWER_ITERATIONS: usize = 8000;

/// Fission-source tolerance of the cold power iteration \[-\].
pub const COLD_FISSION_SOURCE_TOL: f64 = 1.0e-8;

/// `k_eff` tolerance of the cold power iteration \[-\].
pub const COLD_K_EFF_TOL: f64 = 1.0e-9;

/// Result of a critical-boron search — the MATLAB `output` struct of
/// `criticalboron_xyz.m`.
#[derive(Debug, Clone)]
pub struct CriticalBoronOutput {
    /// Critical boron concentration \[ppm\].
    pub boron: f64,
    /// Multiplication factor at that concentration \[-\].
    pub k_eff: f64,
    /// Boron iterates \[ppm\], Phase 1 followed by Phase 2.
    pub boron_history: Vec<f64>,
    /// `k_eff` at each iterate \[-\].
    pub k_eff_history: Vec<f64>,
    /// Measured boron worth \[pcm/ppm\] — negative for a PWR.
    pub slope_pcm_per_ppm: f64,
    /// Converged scalar flux, `state_len` entries.
    pub scalar_flux: Vec<f64>,
    /// Fission source `sigma_f * phi`, `state_len` entries.
    pub fission_source: Vec<f64>,
    /// Node power density `fission_source .* Vi`, `state_len` entries.
    pub pwrdens: Vec<f64>,
    /// Coupled thermal-hydraulic state at critical boron.
    pub th: ThermalState,
    /// Whether both the `k_eff` and the fuel-temperature criteria were met.
    pub converged: bool,
}

/// Search for the boron concentration that makes the coupled steady state
/// critical.
///
/// MATLAB `criticalboron_xyz(geometry, params, th, sigmavalues, whichsigma,
/// initial_k_eff)`.
///
/// # Arguments
///
/// - `initial_k_eff` — MATLAB `varargin{1}`; 1.0 when [`None`].
///
/// Controls read from `params`: [`crit_tol`](CaseParams::crit_tol) (default
/// 1e-5), [`fuel_temp_tol`](CaseParams::fuel_temp_tol) (default 0.5 K),
/// [`th_relax`](CaseParams::th_relax) (default 0.5), and
/// [`boron`](CaseParams::boron) as the starting concentration.
///
/// # Deviations from the MATLAB
///
/// - **No steady-state cache.** `params.steadyfile` names a `.mat` file; there
///   is no `.mat` support here, so Phase 0 always solves.
/// - **No progress printing.** The same iterates are in
///   [`boron_history`](CriticalBoronOutput::boron_history) and
///   [`k_eff_history`](CriticalBoronOutput::k_eff_history).
/// - The MATLAB's two `warning()` calls (a discarded bad cache, and a bootstrap
///   that has not converged) have no output channel here; the second is visible
///   as [`converged`](CriticalBoronOutput::converged) being false.
///
/// # Errors
///
/// [`CouplingError::EigenvalueOutOfRange`] if any eigensolve leaves the sane
/// band — the MATLAB `criticalboron_xyz:badeig` and `:badboot` errors — plus
/// any sparse or feedback failure.
///
/// # Panics
///
/// Through the [`seam`] stubs until `nodal/` and `th/` land.
#[allow(clippy::too_many_lines)]
pub fn search_critical_boron(
    geometry: &CoreGeometry,
    params: &CaseParams,
    th_in: &ThermalState,
    sigma_values: &SigmaValues,
    which_sigma: &MaterialMap,
    initial_k_eff: Option<f64>,
) -> Result<CriticalBoronOutput> {
    let grid = params.grid;
    let philen = grid.state_len();
    let vi_per_group = seam::replicate_per_group(&grid, &geometry.base.volume);
    let initial_k_eff = initial_k_eff.unwrap_or(1.0);

    let crit_tol = params.crit_tol.unwrap_or(DEFAULT_CRIT_TOL);
    let fuel_temp_tol = params.fuel_temp_tol.unwrap_or(DEFAULT_FUEL_TEMP_TOL);
    let w_relax = params.th_relax.unwrap_or(DEFAULT_TH_RELAX);

    let sigma_values_ref = sigma_values;
    let which_sigma_ref = which_sigma;

    // =================================================================== //
    // ----- Phase 0: coupled steady state at the starting boron ----- //
    // =================================================================== //

    let attempt = solve_coupled_steady(
        geometry,
        params,
        th_in,
        sigma_values_ref,
        which_sigma_ref,
        Some(initial_k_eff),
    )?;

    let (mut th, mut phi, mut k_eff) = if in_range(attempt.k_eff, SANE_K_EFF) {
        (attempt.th, attempt.scalar_flux, attempt.k_eff)
    } else {
        // The cold-started Picard of the standard solver can go chaotic at
        // off-nominal conditions. Bootstrap the coupled state robustly instead:
        // flat initial T-H, frozen-nodal power-iteration eigensolves, and
        // under-relaxed static T-H updates at FIXED boron.
        let nodes = grid.nodes();
        let n_solution_ids = fuel_solution_id_count(params, geometry);
        let mut th_b = th_in.clone();
        th_b.fuel_temp_avg = vec![params.fuel_temp_avg_init; nodes];
        th_b.fuel_temp_doppler = vec![params.fuel_temp_avg_init; nodes];
        th_b.fuel_temp = vec![params.fuel_temp_avg_init; nodes * n_solution_ids];
        th_b.n_solution_ids = n_solution_ids;
        th_b.coolant.temps = vec![params.cool_temp_avg_init; nodes];
        th_b.coolant.dens = vec![params.cool_den_avg_init; nodes];
        th_b.heat_flux = vec![0.0; nodes];

        let mut phi_b = vec![1.0_f64; philen];
        let mut k_eff_b = initial_k_eff;
        let mut nodal_terms_b = NodalTerms::zeros(philen);
        let mut k_eff_previous = f64::INFINITY;
        let mut fuel_temp_error_b = f64::INFINITY;

        for _ in 0..MAX_BOOTSTRAP_ITERATIONS {
            let cold = eigensolve_cold(
                params,
                geometry,
                sigma_values_ref,
                which_sigma_ref,
                &th_b,
                &phi_b,
                k_eff_b,
                params.boron,
                &nodal_terms_b,
            )?;
            k_eff_b = cold.k_eff;
            phi_b = cold.flux;
            nodal_terms_b = cold.nodal_terms;
            if !in_range(k_eff_b, SANE_K_EFF_BOOTSTRAP) {
                return Err(CouplingError::EigenvalueOutOfRange {
                    k_eff: k_eff_b,
                    boron: params.boron,
                });
            }
            let th_old = th_b.clone();
            let pwrdens: Vec<f64> = cold
                .fission_source
                .iter()
                .zip(vi_per_group.iter())
                .map(|(f, v)| f * v)
                .collect();
            // NOTE: the bootstrap hands the T-H solver `whichsigmaref` — the
            // *composition* map — where `thdiffusion_solverxyz` hands it the
            // compacted per-node map. Translated as-is; whether the T-H solver
            // cares is a question for `reference::th`.
            th_b = seam::solve_thermal_hydraulics_steady(
                params,
                geometry,
                &th_b,
                which_sigma_ref,
                &pwrdens,
            );
            under_relax(&mut th_b.coolant.dens, &th_old.coolant.dens, w_relax);
            under_relax(
                &mut th_b.fuel_temp_doppler,
                &th_old.fuel_temp_doppler,
                w_relax,
            );
            under_relax(&mut th_b.fuel_temp_avg, &th_old.fuel_temp_avg, w_relax);
            under_relax(&mut th_b.heat_flux, &th_old.heat_flux, w_relax);
            fuel_temp_error_b = max_abs_difference(&th_b.fuel_temp_avg, &th_old.fuel_temp_avg);
            if fuel_temp_error_b < fuel_temp_tol && (k_eff_b - k_eff_previous).abs() < 1.0e-6 {
                break;
            }
            k_eff_previous = k_eff_b;
        }
        let _ = fuel_temp_error_b; // MATLAB warns here; nothing to warn to.
        (th_b, phi_b, k_eff_b)
    };

    // =================================================================== //
    // ----- Phase 1: frozen-T-H secant on static eigensolves ----- //
    // =================================================================== //

    let mut boron_history = vec![0.0_f64; MAX_COUPLED_ITERATIONS];
    let mut k_eff_history = vec![0.0_f64; MAX_COUPLED_ITERATIONS];

    set_at(&mut boron_history, 1, params.boron);
    let mut solution = eigensolve_at_boron(
        params,
        geometry,
        sigma_values_ref,
        which_sigma_ref,
        &th,
        &phi,
        k_eff,
        boron_history[0],
    )?;
    set_at(&mut k_eff_history, 1, solution.k_eff);
    phi = solution.flux.clone();
    let mut fission_source = solution.fission_source.clone();

    let mut slope = DEFAULT_BORON_WORTH_SLOPE;
    let mut it = 1_usize;
    if (k_eff_history[0] - 1.0).abs() >= SECANT_TOL {
        let next_boron = boron_history[0] + (1.0 - k_eff_history[0]) / DEFAULT_BORON_WORTH_SLOPE;
        set_at(&mut boron_history, 2, next_boron);
        for secant_it in 2..=MAX_SECANT_ITERATIONS {
            it = secant_it;
            let boron_here = boron_history[secant_it - 1];
            solution = eigensolve_at_boron(
                params,
                geometry,
                sigma_values_ref,
                which_sigma_ref,
                &th,
                &phi,
                k_eff_history[secant_it - 2],
                boron_here,
            )?;
            set_at(&mut k_eff_history, secant_it, solution.k_eff);
            phi = solution.flux.clone();
            fission_source = solution.fission_source.clone();

            let k_here = k_eff_history[secant_it - 1];
            let k_previous = k_eff_history[secant_it - 2];
            if !in_range(k_here, SANE_K_EFF) {
                return Err(CouplingError::EigenvalueOutOfRange {
                    k_eff: k_here,
                    boron: boron_here,
                });
            }
            if (k_here - k_previous).abs() > 0.0 {
                slope = (k_here - k_previous) / (boron_here - boron_history[secant_it - 2]);
            }
            if (k_here - 1.0).abs() < SECANT_TOL {
                break;
            }
            let next_boron = boron_here
                + (1.0 - k_here) * (boron_history[secant_it - 2] - boron_here)
                    / (k_previous - k_here);
            set_at(&mut boron_history, secant_it + 1, next_boron);
        }
    }
    let mut boron = boron_history[it - 1];
    let n_secant = it;

    // =================================================================== //
    // ----- Phase 2: warm-started coupled boron/flux/T-H loop ----- //
    // =================================================================== //

    let mut fuel_temp_error = f64::INFINITY;
    k_eff = k_eff_history[n_secant - 1];
    let mut coupled_it = 1_usize;
    for outer in 1..=MAX_COUPLED_ITERATIONS {
        coupled_it = outer;
        solution = eigensolve_at_boron(
            params,
            geometry,
            sigma_values_ref,
            which_sigma_ref,
            &th,
            &phi,
            k_eff,
            boron,
        )?;
        k_eff = solution.k_eff;
        phi = solution.flux.clone();
        fission_source = solution.fission_source.clone();
        if !in_range(k_eff, SANE_K_EFF) {
            return Err(CouplingError::EigenvalueOutOfRange { k_eff, boron });
        }
        set_at(&mut boron_history, n_secant + outer, boron);
        set_at(&mut k_eff_history, n_secant + outer, k_eff);

        if (k_eff - 1.0).abs() < crit_tol && fuel_temp_error < fuel_temp_tol {
            break;
        }

        // Boron correction with the measured worth slope.
        boron -= (k_eff - 1.0) / slope;

        // One under-relaxed static T-H update with the current power shape.
        let th_old = th.clone();
        let pwrdens: Vec<f64> = fission_source
            .iter()
            .zip(vi_per_group.iter())
            .map(|(f, v)| f * v)
            .collect();
        th =
            seam::solve_thermal_hydraulics_steady(params, geometry, &th, which_sigma_ref, &pwrdens);
        under_relax(&mut th.coolant.dens, &th_old.coolant.dens, w_relax);
        under_relax(
            &mut th.fuel_temp_doppler,
            &th_old.fuel_temp_doppler,
            w_relax,
        );
        under_relax(&mut th.fuel_temp_avg, &th_old.fuel_temp_avg, w_relax);
        under_relax(&mut th.heat_flux, &th_old.heat_flux, w_relax);
        fuel_temp_error = max_abs_difference(&th.fuel_temp_avg, &th_old.fuel_temp_avg);
    }

    // ----- output ----- //
    let converged = (k_eff - 1.0).abs() < crit_tol && fuel_temp_error < fuel_temp_tol;
    let kept = n_secant + coupled_it;
    boron_history.truncate(kept);
    k_eff_history.truncate(kept);

    let pwrdens: Vec<f64> = fission_source
        .iter()
        .zip(vi_per_group.iter())
        .map(|(f, v)| f * v)
        .collect();

    Ok(CriticalBoronOutput {
        boron,
        k_eff,
        boron_history,
        k_eff_history,
        slope_pcm_per_ppm: slope * 1.0e5,
        scalar_flux: phi,
        fission_source,
        pwrdens,
        th,
        converged,
    })
}

/// What an eigensolve inside the search returns.
#[derive(Debug, Clone)]
pub struct BoronEigenSolution {
    /// Multiplication factor \[-\].
    pub k_eff: f64,
    /// Converged flux, `state_len` entries.
    pub flux: Vec<f64>,
    /// Fission source `sigma_f * phi`, `state_len` entries.
    pub fission_source: Vec<f64>,
}

/// The same, plus the nodal terms the cold bootstrap carries between calls.
#[derive(Debug, Clone)]
pub struct ColdEigenSolution {
    /// Multiplication factor \[-\].
    pub k_eff: f64,
    /// Converged flux, `state_len` entries.
    pub flux: Vec<f64>,
    /// Fission source `sigma_f * phi`, `state_len` entries.
    pub fission_source: Vec<f64>,
    /// Nodal terms, warm-started into the next bootstrap iteration.
    pub nodal_terms: NodalTerms,
}

/// Static eigenvalue at a given boron and **frozen** T-H state, warm-started
/// from the incoming flux.
///
/// MATLAB local function `eigsolveboron`. Updates the cross sections for this
/// boron and T-H state (boron plus Doppler/coolant feedback through
/// [`update_cross_sections`]), then delegates the eigenvalue solve to the
/// production SA-nodal eigensolver. Using the same eigensolver as the Phase-0
/// coupled solve and the transient solver keeps the reported `k_eff` consistent
/// across the whole search.
///
/// # Precondition
///
/// **Only safe from a warm flux.** The production solver's continuous nodal
/// updates act on the flux at every update; from a flat cold flux they diverge,
/// which is why the Phase-0 bootstrap uses [`eigensolve_cold`] instead.
///
/// # Forced parameters
///
/// `params.boron` is set to `boron`, `params.plotfig` to 0 (suppressing the
/// solver's per-call diagnostic figure — no-op here, as nothing plots), and
/// `params.innertol` to [`SEARCH_INNER_TOL`].
///
/// # Errors
///
/// Propagates cross-section feedback failures.
///
/// # Panics
///
/// Through the [`seam`] stub until `nodal/` lands.
// Arity mirrors the MATLAB local function `eigsolveboron`.
#[allow(clippy::too_many_arguments)]
pub fn eigensolve_at_boron(
    params: &CaseParams,
    geometry: &CoreGeometry,
    sigma_values_ref: &SigmaValues,
    which_sigma_ref: &MaterialMap,
    th: &ThermalState,
    flux: &[f64],
    k_eff: f64,
    boron: f64,
) -> Result<BoronEigenSolution> {
    let mut params = params.clone();
    params.boron = boron;
    params.inner_tol = Some(SEARCH_INNER_TOL);

    let (sigma_values_t, which_sigma_t) =
        update_cross_sections(&params, geometry, sigma_values_ref, which_sigma_ref, th)?;
    let out = seam::solve_sanodal_eigenvalue(
        geometry,
        &params,
        &sigma_values_t,
        &which_sigma_t,
        k_eff,
        Some(flux),
    );
    Ok(BoronEigenSolution {
        k_eff: out.k_eff,
        flux: out.scalar_flux,
        fission_source: out.fission_source,
    })
}

/// Robust cold-start eigenvalue solve — Phase 0 bootstrap only.
///
/// MATLAB local function `eigsolvecold`. Builds the operator and the SA-nodal
/// correction from the **incoming** flux with
/// [`COLD_NODAL_REFINEMENTS`] refinements, **freezes** the correction, and runs
/// a self-normalising power iteration on a single cached sparse LU
/// factorisation:
///
/// ```text
/// phi_new  = M \ (fs / k)                      (fixinfnan'd)
/// fs_new   = sigma_f * phi_new
/// k_new    = k * ‖fs_new‖₁ / ‖fs‖₁
/// scale so that sum(fs_new) matches its initial value
/// ```
///
/// converged when the fission-source residual is below
/// [`COLD_FISSION_SOURCE_TOL`] and the `k_eff` residual below
/// [`COLD_K_EFF_TOL`].
///
/// `nodal_terms` is carried across calls so the correction warm-starts as the
/// bootstrap's flux and T-H converge.
///
/// # Note
///
/// Unlike [`eigensolve_at_boron`], this does **not** set `params.innertol` —
/// it does not call the production solver at all.
///
/// # Errors
///
/// Propagates cross-section feedback failures and
/// [`CouplingError::Singular`] if the frozen operator cannot be factorised.
///
/// # Panics
///
/// Through the [`seam`] stubs until `nodal/` lands.
// Arity mirrors the MATLAB local function `eigsolvecold`.
#[allow(clippy::too_many_arguments)]
pub fn eigensolve_cold(
    params: &CaseParams,
    geometry: &CoreGeometry,
    sigma_values_ref: &SigmaValues,
    which_sigma_ref: &MaterialMap,
    th: &ThermalState,
    flux: &[f64],
    k_eff: f64,
    boron: f64,
    nodal_terms: &NodalTerms,
) -> Result<ColdEigenSolution> {
    let mut params = params.clone();
    params.boron = boron;
    let mut geom_t = geometry.clone();

    let (sigma_values_t, which_sigma_t) =
        update_cross_sections(&params, &geom_t, sigma_values_ref, which_sigma_ref, th)?;
    let sigma = seam::make_sigma_operators(&params, &sigma_values_t, &which_sigma_t);
    let diffusion = seam::calc_diffusion_coefficients(&params, &sigma_values_t.tot, &which_sigma_t);
    let (grad_d, gradient_terms) =
        seam::make_gradient_diffusion_operator(&geom_t, &params, &diffusion, &which_sigma_t);
    geom_t.nodal_coeffs = seam::calc_nodal_coefficients(&params, &geom_t, &sigma, &diffusion);

    let mut nodal_terms = nodal_terms.clone();
    let mut nodal = None;
    for _ in 0..COLD_NODAL_REFINEMENTS {
        let (n, nt) = seam::calc_semi_analytic_nodal(
            &params,
            &geom_t,
            flux,
            &sigma,
            &diffusion,
            &gradient_terms,
            &nodal_terms,
            k_eff,
        );
        nodal = Some(n);
        nodal_terms = nt;
    }
    let nodal = nodal.ok_or(CouplingError::MissingCaseData {
        what: "eigensolve_cold: COLD_NODAL_REFINEMENTS must be at least 1",
    })?;

    let m_operator = diffusion_operator(&grad_d, &nodal, &sigma.tot, &sigma.s)?;
    let lu = SparseLu::factorise(&m_operator)?;

    let mut phi = flux.to_vec();
    let mut fs = spmv(&sigma.f, &phi);
    let fs_norm_0 = sum(&fs);
    let mut k_eff = k_eff;
    for _ in 0..COLD_MAX_POWER_ITERATIONS {
        let rhs: Vec<f64> = fs.iter().map(|v| v / k_eff).collect();
        let mut phi_new = lu.solve(&rhs);
        fix_inf_nan(&mut phi_new);
        let mut fs_new = spmv(&sigma.f, &phi_new);
        let k_eff_new = k_eff * norm1(&fs_new) / norm1(&fs);
        let scale = fs_norm_0 / sum(&fs_new);
        for v in &mut phi_new {
            *v *= scale;
        }
        for v in &mut fs_new {
            *v *= scale;
        }
        let difference: Vec<f64> = fs_new.iter().zip(fs.iter()).map(|(a, b)| a - b).collect();
        let residual = norm2(&difference) / norm2(&fs);
        let k_residual = (k_eff_new - k_eff).abs() / k_eff;
        phi = phi_new;
        fs = fs_new;
        k_eff = k_eff_new;
        if residual < COLD_FISSION_SOURCE_TOL && k_residual < COLD_K_EFF_TOL {
            break;
        }
    }

    Ok(ColdEigenSolution {
        k_eff,
        flux: phi,
        fission_source: fs,
        nodal_terms,
    })
}

/// Whether `value` lies strictly inside `range` and is finite.
///
/// MATLAB `isfinite(k) && k > lo && k < hi`, the sane-range guard repeated at
/// every eigensolve.
#[must_use]
fn in_range(value: f64, range: (f64, f64)) -> bool {
    value.is_finite() && value > range.0 && value < range.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sane_range_rejects_nan_and_the_endpoints() {
        assert!(in_range(1.0, SANE_K_EFF));
        assert!(!in_range(f64::NAN, SANE_K_EFF));
        assert!(!in_range(0.8, SANE_K_EFF));
        assert!(!in_range(1.2, SANE_K_EFF));
        // The bootstrap guard is deliberately wider.
        assert!(in_range(1.4, SANE_K_EFF_BOOTSTRAP));
        assert!(!in_range(1.4, SANE_K_EFF));
    }

    #[test]
    fn the_boron_worth_seed_moves_boron_the_right_way() {
        // A supercritical core (k > 1) must call for MORE boron.
        let k = 1.01_f64;
        let boron_0 = 1000.0_f64;
        let boron_1 = boron_0 + (1.0 - k) / DEFAULT_BORON_WORTH_SLOPE;
        assert!(boron_1 > boron_0, "got {boron_1}");
        // ...and a subcritical one for less.
        let k = 0.99_f64;
        let boron_1 = boron_0 + (1.0 - k) / DEFAULT_BORON_WORTH_SLOPE;
        assert!(boron_1 < boron_0, "got {boron_1}");
    }
}
