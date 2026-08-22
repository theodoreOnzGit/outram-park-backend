//! Critical-boron search for the coupled neutronics / thermal-hydraulic
//! steady state.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `criticalboron_xyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What it does, in three phases
//!
//! Finds the boron concentration at which the **coupled** steady state is
//! critical. The reference's header records that this file was rewritten in
//! June 2026 after the obvious approach failed, and the failure is worth
//! knowing because the structure exists entirely to avoid it:
//!
//! > The previous implementation wrapped a secant iteration around full
//! > **cold-started** coupled solves. The cold-start T-H Picard can go chaotic
//! > at off-nominal boron — `k_eff` transients into the hundreds — and either
//! > trips the solver's not-converging exit, returning a garbage `k_eff` that
//! > poisons the secant (**observed: boron diverging past 1e5 ppm**), or
//! > settles into a spurious coupled state.
//!
//! So the rewrite never cold-starts the thermal-hydraulics away from the
//! starting boron:
//!
//! 1. **Phase 0** — one coupled steady solve at the starting boron. If the
//!    standard solver diverges from its cold start, a bootstrap loop recovers
//!    a usable coupled state using frozen-nodal eigensolves.
//! 2. **Phase 1** — a guarded secant on **static** eigensolves at the frozen
//!    Phase-0 T-H state. Cheap, and it measures the boron worth slope.
//! 3. **Phase 2** — a warm-started coupled loop: one static eigensolve per
//!    outer iteration, a boron correction using the measured slope, and one
//!    under-relaxed static T-H update. Boron, flux and feedback converge
//!    together.
//!
//! # Why there are two different eigensolvers
//!
//! This is the subtlest part of the file, and the reference documents it at
//! length because both halves were established by experiment.
//!
//! `eigsolve_boron` delegates to [`crate::sanodaldiffusion_solverxyz`] — the
//! same eigensolver the steady and transient drivers use, so the reported
//! `k_eff` stays consistent across the whole search. That is safe **only**
//! because the flux is warm-started from a good shape: the solver's continuous
//! nodal updates then act on a good flux at every update and stay stable.
//!
//! `eigsolve_cold` exists because from a **flat** cold flux they do not. The
//! reference records two verified findings:
//!
//! - `sanodaldiffusion`'s continuous nodal updates use the still-bad
//!   mid-iteration flux on a cold start and **diverge to `k_eff` around 5e4**
//!   on the heavily-rodded configuration. Freezing them via a huge `nodalupd`
//!   does stabilise the cold solve in isolation — but
//! - `sanodaldiffusion` builds its *initial* nodal correction from a **flat**
//!   flux (hardcoded ones), so a frozen call returns a roughly **25 pcm-biased,
//!   flatter seed**, and that poorer seed then destabilised a near-critical
//!   Phase-1 warm solve (**`k_eff` to 377**).
//!
//! `eigsolve_cold` therefore builds the nodal correction from the **warm**
//! flux, freezes it, and power-iterates — stable cold *and* an accurate seed,
//! which the production solver cannot be made to do through its parameters.
//!
//! # One deliberate departure from the reference
//!
//! **The `.mat` steady-state cache is not translated.** As in
//! [`crate::thdiffusion_solvertimexyz`], `params.steadyfile` becomes an
//! explicit `initial_steady` argument so the caller owns cache invalidation.
//! The reference additionally *validates* its cache — discarding it with a
//! warning if the stored `k_eff` is outside `[0.8, 1.2]` — which an explicit
//! argument makes unnecessary.
//!
//! # What this can be checked against
//!
//! [`crate::neacrpa2t`] records the only published NEACRP number in the
//! snapshot: case A2's critical boron is **1160.6 ppm** (PANTHER,
//! NEA/NSC/DOC(93)25 Table 3.1), against the **1139.01 ppm** the reference
//! computes for itself. This module is what would produce that second number —
//! but the search that originally produced it, `test_critboron3.m`, is **not in
//! the snapshot**, so its settings are unknown. See
//! `docs/bedok-reference-defects.md`, "Missing files".
//!
//! # OPEN DISCREPANCY — read before using this module's answer
//!
//! Run on [`crate::neacrpa2`], this port converges to **1253.29 ppm**
//! (`k_eff` = 1.000001), against the reference's 1139.01 ppm. At the measured
//! boron worth of -9.62 pcm/ppm that gap is about **1100 pcm** — far beyond
//! round-off. **This port computes a materially more reactive core than the
//! MATLAB does.**
//!
//! **The cause is now narrowed.** Running the identical search on
//! [`crate::neacrpa1t`] — the same core at hot zero power — reproduces *that*
//! case's reference value to **0.03%** (551.14 against 551.31 ppm, under 2 pcm),
//! and on that run Phase 0 did **not** need the bootstrap. A1 and A2 share the
//! cross sections, the feedback chain, the eigensolver and this whole search, so
//! a mistranslation in any of them is very unlikely.
//!
//! What differs is the Phase-0 path: **A2 fell back to the bootstrap and A1 did
//! not.** The open question is therefore either that the bootstrap converges a
//! different coupled state, or that [`crate::thdiffusion_solverxyz`] fails on A2
//! where the MATLAB succeeds — in which case the bootstrap is merely exposing a
//! defect in the coupled driver.
//!
//! Measured 2026-08-18; the full breakdown is in that module test. **Until this
//! is settled, do not trust this module's answer on a case that reports
//! `bootstrapped == true`.**

use crate::error::{BedokError, Result};
use crate::matlab::{norm1, norm2, Array2, Array3, Decomposition, SparseMatrix};
use crate::sigmavalupd3d_handler::{sigmavalupd3d_handler, FeedbackTables};
use crate::thdiffusion_solverxyz::{thdiffusion_solverxyz, CoupledOutput};
use crate::types::{Geometry, Params, SigmaValues, Th};

/// The reference's defaults for the search.
pub mod defaults {
    /// `crittol` — tolerance on `|k_eff - 1|` for the critical state.
    pub const CRIT_TOL: f64 = 1e-5;
    /// `fueltemptol` — fuel-temperature convergence tolerance, K.
    pub const FUELTEMP_TOL: f64 = 0.5;
    /// `threlax` — T-H Picard under-relaxation factor.
    pub const RELAX: f64 = 0.5;
    /// `slopedefault` — a typical PWR boron worth, `dk/db` per ppm, used to
    /// seed the secant before any slope has been measured.
    pub const SLOPE_SEED: f64 = -9e-5;
    /// `maxout` — Phase-2 outer iterations.
    pub const MAX_OUTER: usize = 40;
    /// The Phase-1 secant iteration cap.
    pub const MAX_SECANT: usize = 12;
    /// The Phase-0 bootstrap iteration cap.
    pub const MAX_BOOTSTRAP: usize = 30;
    /// Power iterations inside `eigsolve_cold`.
    pub const COLD_POWER_ITER: usize = 8000;
    /// Nodal refinements `eigsolve_cold` applies before freezing.
    pub const COLD_NODAL_REFINE: usize = 3;
    /// The tight inner tolerance the search eigensolves use, for a
    /// sub-ppm-accurate critical `k_eff`.
    pub const SEARCH_INNER_TOL: f64 = 1e-8;
    /// The Phase-1 secant's own convergence test on `|k_eff - 1|`.
    ///
    /// Looser than [`CRIT_TOL`] because Phase 2 refines it afterwards.
    pub const SECANT_TOL: f64 = 2e-6;
}

/// A `k_eff` outside `[lo, hi]` aborts the search rather than poisoning it.
fn guard(k_eff: f64, boron: f64, lo: f64, hi: f64, phase: &'static str) -> Result<()> {
    if !k_eff.is_finite() || k_eff < lo || k_eff > hi {
        return Err(BedokError::BoronSearchDiverged { k_eff, boron, phase });
    }
    Ok(())
}

/// What the search returns.
#[derive(Clone, Debug)]
pub struct BoronOutput {
    /// `output.boron` — the critical concentration, ppm.
    pub boron: f64,
    /// `output.k_eff` at that concentration.
    pub k_eff: f64,
    /// `output.boronhist` — every concentration tried, in order.
    pub boron_history: Vec<f64>,
    /// `output.keffhist` — the matching eigenvalues.
    pub k_eff_history: Vec<f64>,
    /// `output.slope_pcm_per_ppm` — the measured boron worth.
    ///
    /// Negative: boron is an absorber, so more of it lowers `k_eff`.
    pub slope_pcm_per_ppm: f64,
    /// **How many bootstrap cold solves hit their 8000-iteration cap without
    /// converging** — defect C7.
    ///
    /// The reference's cold power iteration returns whatever it holds when the
    /// counter runs out, with no error and no flag, so the bootstrap can build
    /// its starting state out of eigenvalues that are not solutions and the
    /// search then hunts around it. **The iteration is unchanged**; this
    /// counts the abandoned ones.
    ///
    /// `0` when the bootstrap was not needed, which is the normal case since
    /// the stage-2 corrections landed — see
    /// `the_search_finds_a_critical_boron_on_the_pwr_case`, which now reports
    /// `bootstrapped == false`.
    pub cold_solves_not_converged: usize,
    /// `output.scalar_flux` at the critical state.
    pub scalar_flux: Vec<f64>,
    /// `output.fission_source` — `sigma.f * phi`.
    pub fission_source: Vec<f64>,
    /// `output.pwrdens` — `fission_source .* Vi`.
    pub pwrdens: Vec<f64>,
    /// `output.th` — the coupled state at the critical boron.
    pub th: Th,
    /// How many Phase-1 secant iterations ran.
    pub secant_iterations: usize,
    /// How many Phase-2 coupled iterations ran.
    pub coupled_iterations: usize,
    /// Whether both criteria — `|k_eff - 1|` and the fuel temperature — were
    /// met. The reference prints `[converged]` / `[NOT converged]` for this.
    pub converged: bool,
    /// Whether Phase 0 had to fall back to the bootstrap loop.
    ///
    /// Not in the reference's output, which only warns. A caller otherwise has
    /// no way to know the standard solver failed.
    pub bootstrapped: bool,
}

/// Static eigenvalue at a given boron and **frozen** T-H state — the warm
/// Phase 1/2 solve.
///
/// Updates the cross sections for this boron and T-H state, then delegates to
/// [`crate::sanodaldiffusion_solverxyz`], warm-started from the incoming flux.
/// See the module docs on why this is safe only when warm-started.
#[allow(clippy::too_many_arguments)]
fn eigsolve_boron(
    params: &Params,
    geometry: &Geometry,
    sigmavaluesref: &SigmaValues,
    feedback: &FeedbackTables,
    whichsigmaref: &Array3<usize>,
    th: &Th,
    phi: &[f64],
    k_eff: f64,
    boron: f64,
) -> Result<(f64, Vec<f64>, Vec<f64>)> {
    let mut p = params.clone();
    p.boron = boron;
    p.plotfig = 0;
    p.innertol = Some(defaults::SEARCH_INNER_TOL);

    let (sv, ws, _rod) =
        sigmavalupd3d_handler(&p, geometry, sigmavaluesref, feedback, whichsigmaref, th)?;

    let mut init = Array2::<f64>::zeros(phi.len(), 1);
    for (i, v) in phi.iter().enumerate() {
        init.set(i, 0, *v);
    }
    let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
        geometry,
        &p,
        &sv,
        &ws,
        Some(k_eff),
        Some(&init),
    )?;

    let flux: Vec<f64> = (0..out.scalar_flux.rows())
        .map(|i| out.scalar_flux.get(i, 0))
        .collect();
    Ok((out.k_eff, flux, out.fission_source))
}

/// Robust cold-start eigenvalue solve — **Phase 0 bootstrap only**.
///
/// Builds the operator and the SA-nodal correction from the *incoming* flux,
/// freezes the correction, and runs a self-normalising power iteration. See
/// the module docs for why neither property can be had from
/// [`crate::sanodaldiffusion_solverxyz`] on a cold start.
///
/// `nodalterms` is carried across calls so the correction warm-starts as the
/// bootstrap's flux and T-H converge.
#[allow(clippy::too_many_arguments)]
fn eigsolve_cold(
    params: &Params,
    geometry: &Geometry,
    sigmavaluesref: &SigmaValues,
    feedback: &FeedbackTables,
    whichsigmaref: &Array3<usize>,
    th: &Th,
    phi: &[f64],
    k_eff: f64,
    boron: f64,
    nodalterms: &mut Array2<f64>,
    buck_cache: &mut crate::calc_bucklingxyz::BucklingCache,
) -> Result<(f64, Vec<f64>, Vec<f64>, ColdSolveVerdict)> {
    let mut p = params.clone();
    p.boron = boron;

    let (sv, ws, _rod) =
        sigmavalupd3d_handler(&p, geometry, sigmavaluesref, feedback, whichsigmaref, th)?;
    let mut sigma = crate::makesigmadfxyz::makesigmadfxyz(&p, &sv, &ws, None);
    let diffd = crate::calcdiffvalues3d::calcdiffvalues3d(&p, &sv.tot, &ws, None);
    let gradd = crate::makegrad_dxyz::makegrad_dxyz(geometry, &p, &diffd, &ws, None)?;
    let geomt = geometry.clone();
    // The reference stores this on `geomt.nodalcoeffs`; this crate passes the
    // coefficients explicitly, so `calc_sanodalxyz` takes them as an argument.
    let coeffs = crate::calc_abefghxyz::calc_abefghxyz(&p, &geomt, &mut sigma, &diffd);

    // Refine the correction from the warm flux, then freeze it.
    let mut sanodal = None;
    for _ in 0..defaults::COLD_NODAL_REFINE {
        let r = crate::calc_sanodalxyz::calc_sanodalxyz(
            &p, &geomt, &coeffs, phi, &mut sigma, &diffd, &gradd.terms, nodalterms, k_eff,
            buck_cache,
        );
        *nodalterms = r.terms.clone();
        sanodal = Some(r);
    }
    let sanodal = sanodal.expect("COLD_NODAL_REFINE must be at least 1");

    let mut m = SparseMatrix::combine(&[
        (&gradd.operator, 1.0),
        (&sanodal.operator, 1.0),
        (&sigma.tot, 1.0),
        (&sigma.s, -1.0),
    ]);
    let dm = Decomposition::new(&mut m);

    let mut phi = phi.to_vec();
    let mut fs = sigma.f.mul_vec(&phi);
    let fsn0: f64 = fs.iter().sum();
    let mut k = k_eff;
    // Defect C7: the reference exits this loop on the cap with no error and no
    // flag, so a non-converged eigenvalue is indistinguishable from a
    // converged one. The iteration is unchanged; what is added is the verdict.
    let mut converged = false;
    let mut last_res = f64::INFINITY;
    let mut last_kres = f64::INFINITY;
    for _ in 0..defaults::COLD_POWER_ITER {
        let rhs: Vec<f64> = fs.iter().map(|x| x / k).collect();
        let mut phin = crate::fixinfnan::fixinfnan(&dm.solve(&rhs), false);
        let mut fsn = sigma.f.mul_vec(&phin);
        let kn = k * norm1(&fsn) / norm1(&fs);
        let sc = fsn0 / fsn.iter().sum::<f64>();
        for x in phin.iter_mut() {
            *x *= sc;
        }
        for x in fsn.iter_mut() {
            *x *= sc;
        }
        let diff: Vec<f64> = fsn.iter().zip(&fs).map(|(a, b)| a - b).collect();
        let res = norm2(&diff) / norm2(&fs);
        let kres = (kn - k).abs() / k;
        phi = phin;
        fs = fsn;
        k = kn;
        last_res = res;
        last_kres = kres;
        if res < 1e-8 && kres < 1e-9 {
            converged = true;
            break;
        }
    }
    Ok((k, phi, fs, ColdSolveVerdict { converged, iterations: defaults::COLD_POWER_ITER, residual: last_res, k_eff_residual: last_kres }))
}

/// Whether the cold power iteration actually converged — defect C7.
///
/// The reference runs its 8000-iteration loop and returns whatever it holds
/// when the counter runs out, with no error and no flag, so a caller cannot
/// tell a converged eigenvalue from an abandoned one. This carries the verdict
/// alongside the answer. **The answer itself is unchanged.**
#[derive(Clone, Copy, Debug)]
pub struct ColdSolveVerdict {
    /// Whether both tolerances were met before the cap.
    ///
    /// **`false` means the eigenvalue and flux are not solutions**, merely
    /// where the iteration happened to stop.
    pub converged: bool,
    /// The cap the loop was allowed.
    pub iterations: usize,
    /// The final relative fission-source residual, `1e-8` to pass.
    pub residual: f64,
    /// The final relative `k_eff` change, `1e-9` to pass.
    pub k_eff_residual: f64,
}

/// Under-relax the four T-H fields the reference relaxes, in place.
///
/// **Only these four.** The reference leaves everything else on `th` — the
/// coolant temperatures, enthalpies, the full radial fuel profile — taken
/// straight from the new solve. Reproduced exactly; see
/// [`crate::thdiffusion_solverxyz`], which relaxes the same four.
fn relax_th(new: &mut Th, old: &Th, w: f64) {
    let blend = |a: &[f64], b: &[f64]| -> Vec<f64> {
        a.iter().zip(b).map(|(o, n)| (1.0 - w) * o + w * n).collect()
    };
    new.coolant.dens = blend(&old.coolant.dens, &new.coolant.dens);
    new.fueltempdoppler = blend(&old.fueltempdoppler, &new.fueltempdoppler);
    new.fueltempavg = blend(&old.fueltempavg, &new.fueltempavg);
    new.heatflux = blend(&old.heatflux, &new.heatflux);
}

/// `output = criticalboron_xyz(geometry, params, th, sigmavalues, whichsigma, varargin)`.
///
/// # Arguments
///
/// - `initial_steady` — a precomputed Phase-0 coupled state. `None` runs
///   Phase 0. Replaces the reference's `params.steadyfile` `.mat` cache.
/// - `initial_k_eff` — `varargin{1}`; the reference defaults it to 1.
///
/// # Errors
///
/// [`BedokError::BoronSearchDiverged`] if any eigensolve leaves the sane range,
/// plus whatever the operator chain and the coupled solver raise.
#[allow(clippy::too_many_arguments)]
pub fn criticalboron_xyz(
    geometry: &Geometry,
    params: &Params,
    th: &Th,
    sigmavaluesref: &SigmaValues,
    feedback: &FeedbackTables,
    whichsigmaref: &Array3<usize>,
    initial_steady: Option<&CoupledOutput>,
    initial_k_eff: Option<f64>,
) -> Result<BoronOutput> {
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let philen = params.g * es;
    let vig: Vec<f64> = (0..philen).map(|i| geometry.vi[i % es]).collect();

    let initial_k_eff = initial_k_eff.unwrap_or(1.0);
    let crittol = params.crittol.unwrap_or(defaults::CRIT_TOL);
    let fueltemptol = params.fueltemptol.unwrap_or(defaults::FUELTEMP_TOL);
    let wrelax = params.threlax.unwrap_or(defaults::RELAX);

    // =================================================================== //
    // Phase 0: coupled steady state at the starting boron
    // =================================================================== //
    let mut bootstrapped = false;
    // Defect C7 — see `BoronOutput::cold_solves_not_converged`.
    let mut cold_solves_not_converged = 0usize;
    let (mut th, mut phi, mut k_eff) = match initial_steady {
        Some(s) => {
            let flux: Vec<f64> = (0..s.scalar_flux.rows()).map(|i| s.scalar_flux.get(i, 0)).collect();
            (s.th.clone(), flux, s.k_eff)
        }
        None => {
            // First try the standard coupled solver once — fast when it works.
            let s = thdiffusion_solverxyz(
                geometry,
                params,
                th,
                sigmavaluesref,
                feedback,
                whichsigmaref,
                Some(initial_k_eff),
            )?;
            if s.k_eff.is_finite() && s.k_eff > 0.8 && s.k_eff < 1.2 {
                let flux: Vec<f64> =
                    (0..s.scalar_flux.rows()).map(|i| s.scalar_flux.get(i, 0)).collect();
                (s.th.clone(), flux, s.k_eff)
            } else {
                // The cold-started Picard went chaotic. Bootstrap the coupled
                // state instead: flat T-H, frozen-nodal eigensolves, and
                // under-relaxed T-H updates at FIXED boron.
                bootstrapped = true;

                let maxir = params.fuel.maxir;
                let whichk = &geometry.fuel.whichk;
                let mut surfcount = 0usize;
                for ir in 0..maxir - 1 {
                    if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                        surfcount += 1;
                    }
                }
                let maxid = maxir + surfcount;

                let mut thb = th.clone();
                thb.fueltempavg = vec![params.fueltempavg; es];
                thb.fueltempdoppler = vec![params.fueltempavg; es];
                thb.fueltemp = {
                    let mut a = Array2::<f64>::zeros(es, maxid);
                    for i in 0..es {
                        for j in 0..maxid {
                            a.set(i, j, params.fueltempavg);
                        }
                    }
                    a
                };
                thb.coolant.temps = vec![params.cooltempavg; es];
                thb.coolant.dens = vec![params.cooldenavg; es];
                thb.heatflux = vec![0.0; es];

                let mut phib = vec![1.0; philen];
                let mut keffb = initial_k_eff;
                let mut ntb = Array2::<f64>::zeros(philen, 6);
                let mut buck = crate::calc_bucklingxyz::BucklingCache::new();
                let mut keffprev = f64::INFINITY;
                let mut fterrb = f64::INFINITY;

                for _ in 0..defaults::MAX_BOOTSTRAP {
                    let (k, p, fsb, cold) = eigsolve_cold(
                        params,
                        geometry,
                        sigmavaluesref,
                        feedback,
                        whichsigmaref,
                        &thb,
                        &phib,
                        keffb,
                        params.boron,
                        &mut ntb,
                        &mut buck,
                    )?;
                    keffb = k;
                    phib = p;
                    // Defect C7: an abandoned cold solve is silent in the
                    // reference. Counted rather than ignored.
                    if !cold.converged {
                        cold_solves_not_converged += 1;
                    }
                    guard(keffb, params.boron, 0.5, 1.5, "bootstrap")?;

                    let pwr: Vec<f64> = fsb.iter().zip(&vig).map(|(a, b)| a * b).collect();
                    let thold = thb.clone();
                    let (mut thnew, _rods) = crate::th_solverxyz::th_solverxyz(
                        params,
                        geometry,
                        &thb,
                        whichsigmaref,
                        &pwr,
                    );
                    relax_th(&mut thnew, &thold, wrelax);
                    thb = thnew;

                    fterrb = thb
                        .fueltempavg
                        .iter()
                        .zip(&thold.fueltempavg)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max);
                    if fterrb < fueltemptol && (keffb - keffprev).abs() < 1e-6 {
                        break;
                    }
                    keffprev = keffb;
                }
                // The reference warns and continues when the bootstrap has not
                // settled — Phase 2 iterates further.
                let _ = fterrb;
                (thb, phib, keffb)
            }
        }
    };

    // =================================================================== //
    // Phase 1: frozen-T-H secant on static eigensolves
    // =================================================================== //
    let mut boron_history: Vec<f64> = Vec::new();
    let mut k_eff_history: Vec<f64> = Vec::new();

    let mut boron = params.boron;
    let (k0, p0, mut fs) = eigsolve_boron(
        params, geometry, sigmavaluesref, feedback, whichsigmaref, &th, &phi, k_eff, boron,
    )?;
    phi = p0;
    boron_history.push(boron);
    k_eff_history.push(k0);

    let mut slope = defaults::SLOPE_SEED;
    let mut nsec = 1usize;

    if (k0 - 1.0).abs() >= defaults::SECANT_TOL {
        let mut b_prev = boron;
        let mut k_prev = k0;
        boron = b_prev + (1.0 - k_prev) / defaults::SLOPE_SEED;

        for it in 2..=defaults::MAX_SECANT {
            nsec = it;
            let (k, p, f) = eigsolve_boron(
                params, geometry, sigmavaluesref, feedback, whichsigmaref, &th, &phi, k_prev,
                boron,
            )?;
            phi = p;
            fs = f;
            boron_history.push(boron);
            k_eff_history.push(k);
            guard(k, boron, 0.8, 1.2, "eigensolve")?;

            if (k - k_prev).abs() > 0.0 {
                slope = (k - k_prev) / (boron - b_prev);
            }
            if (k - 1.0).abs() < defaults::SECANT_TOL {
                k_prev = k;
                break;
            }
            // Secant step.
            let next = boron + (1.0 - k) * (b_prev - boron) / (k_prev - k);
            b_prev = boron;
            k_prev = k;
            boron = next;
        }
        k_eff = k_prev;
    } else {
        k_eff = k0;
    }

    // =================================================================== //
    // Phase 2: warm-started coupled boron / flux / T-H loop
    // =================================================================== //
    let mut fterr = f64::INFINITY;
    let mut coupled_iterations = 0usize;

    for it in 1..=defaults::MAX_OUTER {
        coupled_iterations = it;
        let (k, p, f) = eigsolve_boron(
            params, geometry, sigmavaluesref, feedback, whichsigmaref, &th, &phi, k_eff, boron,
        )?;
        k_eff = k;
        phi = p;
        fs = f;
        guard(k_eff, boron, 0.8, 1.2, "eigensolve")?;
        boron_history.push(boron);
        k_eff_history.push(k_eff);

        if (k_eff - 1.0).abs() < crittol && fterr < fueltemptol {
            break;
        }

        // Boron correction with the measured worth slope.
        boron -= (k_eff - 1.0) / slope;

        // One under-relaxed static T-H update at the current power shape.
        let pwr: Vec<f64> = fs.iter().zip(&vig).map(|(a, b)| a * b).collect();
        let thold = th.clone();
        let (mut thnew, _rods) =
            crate::th_solverxyz::th_solverxyz(params, geometry, &th, whichsigmaref, &pwr);
        relax_th(&mut thnew, &thold, wrelax);
        th = thnew;

        fterr = th
            .fueltempavg
            .iter()
            .zip(&thold.fueltempavg)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
    }

    let converged = (k_eff - 1.0).abs() < crittol && fterr < fueltemptol;
    let pwrdens: Vec<f64> = fs.iter().zip(&vig).map(|(a, b)| a * b).collect();

    Ok(BoronOutput {
        cold_solves_not_converged,
        boron,
        k_eff,
        boron_history,
        k_eff_history,
        slope_pcm_per_ppm: slope * 1e5,
        scalar_flux: phi,
        fission_source: fs,
        pwrdens,
        th,
        secant_iterations: nsec,
        coupled_iterations,
        converged,
        bootstrapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sanity guard rejects a poisoned eigenvalue instead of searching on.
    ///
    /// # Methodology
    ///
    /// This is the whole reason the June 2026 rewrite exists: an earlier
    /// version fed garbage eigenvalues into the secant and watched boron
    /// diverge past 1e5 ppm. The guard is the mechanism that stops it, so it is
    /// tested directly at both phases' bounds — `[0.8, 1.2]` for a search
    /// eigensolve, `[0.5, 1.5]` for the bootstrap — including the `NaN` case,
    /// which a plain range comparison would let through.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// All eight rejections fired, including `NaN` and `+inf`, and the
    /// message carries the phase and both numbers:
    ///
    /// ```text
    /// critical-boron eigensolve returned k_eff = 50000 at 1234.5 ppm, outside the sane range
    /// ```
    ///
    /// **Interpretation.** The 5e4 case is the exact value the reference
    /// records the production eigensolver reaching on a cold start, so the
    /// guard demonstrably catches the failure it was written for. The `NaN`
    /// case matters most: `NaN < 0.8` and `NaN > 1.2` are both false, so a
    /// range test without the `is_finite` check would pass it straight into
    /// the secant — which is how boron reached 1e5 ppm in the first place.
    #[test]
    fn the_sanity_guard_rejects_poisoned_eigenvalues() {
        // Inside the search range.
        assert!(guard(1.0, 1000.0, 0.8, 1.2, "eigensolve").is_ok());
        assert!(guard(0.81, 1000.0, 0.8, 1.2, "eigensolve").is_ok());

        // Outside it, in both directions.
        for bad in [0.79, 1.21, 5e4, -1.0, 0.0] {
            let e = guard(bad, 1234.5, 0.8, 1.2, "eigensolve").unwrap_err();
            eprintln!("k_eff = {bad}: {e}");
            assert!(matches!(e, BedokError::BoronSearchDiverged { .. }));
        }

        // NaN must be caught — `NaN < 0.8` and `NaN > 1.2` are both false, so a
        // bare range test would pass it through.
        let e = guard(f64::NAN, 900.0, 0.8, 1.2, "eigensolve").unwrap_err();
        eprintln!("k_eff = NaN: {e}");
        assert!(matches!(e, BedokError::BoronSearchDiverged { .. }));
        assert!(guard(f64::INFINITY, 900.0, 0.8, 1.2, "eigensolve").is_err());

        // The bootstrap band is wider.
        assert!(guard(0.6, 1000.0, 0.5, 1.5, "bootstrap").is_ok());
        assert!(guard(0.6, 1000.0, 0.8, 1.2, "eigensolve").is_err());

        // The error names the phase and both numbers.
        let e = guard(0.5, 777.0, 0.8, 1.2, "eigensolve").unwrap_err();
        let msg = e.to_string();
        eprintln!("message: {msg}");
        assert!(msg.contains("eigensolve") && msg.contains("777"));
    }

    /// Under-relaxation blends exactly the four fields the reference blends.
    ///
    /// # Methodology
    ///
    /// `criticalboron_xyz.m` relaxes `coolant.dens`, `fueltempdoppler`,
    /// `fueltempavg` and `heatflux` — and **nothing else**, so the coolant
    /// temperatures and the radial fuel profile come through from the new solve
    /// unrelaxed. That asymmetry is easy to "tidy up" by accident, so it is
    /// pinned: with `w = 0.5` the four blend to the midpoint and the fifth
    /// field must be untouched.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// At `w = 0.5` the four relaxed fields blended to their midpoints
    /// (density 1.5, Doppler 700, fuel average 800, heat flux 20) and the
    /// coolant temperature came through **unrelaxed at 900**.
    ///
    /// **Interpretation.** The asymmetry is reproduced exactly. It is not
    /// obviously intentional in the reference — relaxing the density but not
    /// the temperature it was computed from is at least odd — but it is what
    /// the reference does, in both this file and
    /// [`crate::thdiffusion_solverxyz`], so it stands.
    #[test]
    fn under_relaxation_touches_only_the_four_reference_fields() {
        let old = Th {
            coolant: crate::types::Coolant {
                dens: vec![1.0, 1.0],
                temps: vec![500.0, 500.0],
                ..Default::default()
            },
            fueltempdoppler: vec![600.0, 600.0],
            fueltempavg: vec![700.0, 700.0],
            heatflux: vec![10.0, 10.0],
            ..Default::default()
        };
        let mut new = Th {
            coolant: crate::types::Coolant {
                dens: vec![2.0, 2.0],
                temps: vec![900.0, 900.0],
                ..Default::default()
            },
            fueltempdoppler: vec![800.0, 800.0],
            fueltempavg: vec![900.0, 900.0],
            heatflux: vec![30.0, 30.0],
            ..Default::default()
        };

        relax_th(&mut new, &old, 0.5);

        eprintln!("dens      {:?} (0.5 * 1 + 0.5 * 2)", new.coolant.dens);
        eprintln!("doppler   {:?}", new.fueltempdoppler);
        eprintln!("fuel avg  {:?}", new.fueltempavg);
        eprintln!("heat flux {:?}", new.heatflux);
        eprintln!("temps     {:?} (NOT relaxed)", new.coolant.temps);

        assert_eq!(new.coolant.dens, vec![1.5, 1.5]);
        assert_eq!(new.fueltempdoppler, vec![700.0, 700.0]);
        assert_eq!(new.fueltempavg, vec![800.0, 800.0]);
        assert_eq!(new.heatflux, vec![20.0, 20.0]);
        // The reference does not relax this one.
        assert_eq!(new.coolant.temps, vec![900.0, 900.0]);

        // w = 0 keeps the old state; w = 1 takes the new one.
        let mut a = new.clone();
        relax_th(&mut a, &old, 0.0);
        assert_eq!(a.fueltempavg, vec![700.0, 700.0]);
    }

    /// **X1 localisation: does the feedback handler agree with the MATLAB?**
    ///
    /// # Methodology
    ///
    /// This isolates [`crate::sigmavalupd3d_handler`] from every solver. Both
    /// codes are given [`crate::neacrpa2`] at **exactly** the initial T-H state
    /// [`crate::thdiffusion_solverxyz`] builds — uniform `fueltempavg`,
    /// `cooltempavg`, `cooldenavg`, zero heat flux — the handler is called once,
    /// and the resulting cross sections are expanded **per node** and summed.
    ///
    /// Per-node rather than per-material-row, so the comparison does not depend
    /// on how either code happens to renumber materials. Six sums: total and
    /// fission in both groups, the down-scatter, and the group-1 diagonal.
    ///
    /// The MATLAB side was run on 2026-08-18 (`x1_dump_sigma.m`, MATLAB
    /// R2026a) and reported **3978 material rows** after renumbering, with the
    /// checksums below.
    ///
    /// If these match, the feedback chain is clean and X1 lives downstream in
    /// the eigensolver or the T-H. If they do not, the failing sum names the
    /// mistranslated quantity.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **This test found defect Z1.** On its first run the six sums agreed to
    /// only 2e-6 - 2e-5, far above round-off, and the rod-fraction map
    /// localised it exactly: both codes gave 270 rodded nodes, 221 full and 49
    /// partial, but **every partial fraction differed by precisely 0.01**.
    /// Working backwards, this port's cumulative axial height at each rod tip
    /// was 0.3 cm short — the silently rounded mesh.
    ///
    /// After reproducing Z1:
    ///
    /// | quantity | rel. difference |
    /// |---|---|
    /// | rod-fraction map | **1.1e-15** |
    /// | material rows | 3978 = 3978 |
    /// | total, groups 1 / 2 | 1.3e-14 / 5.9e-14 |
    /// | fission, groups 1 / 2 | **3.3e-13** / 1.7e-14 |
    /// | down-scatter / diagonal | 7.3e-14 / 2.0e-14 |
    ///
    /// **Interpretation.** The whole feedback chain — all five channels, the
    /// rod tip search, the material renumbering — now agrees with the MATLAB to
    /// floating-point round-off. Whatever remains of X1 is not here.
    #[test]
    fn x1_does_the_feedback_handler_match_the_matlab() {
        // From x1_dump_sigma.m, MATLAB R2026a, 2026-08-18.
        const MATLAB_ROWS: usize = 3978;
        const MATLAB: [(&str, f64); 6] = [
            ("tot g1", 9.279882816892e+02),
            ("tot g2", 4.839652876719e+03),
            ("f   g1", 1.400345031619e+01),
            ("f   g2", 2.535798398243e+02),
            ("s 2<-1", 8.006400955716e+01),
            ("s 1<-1", 8.227218056848e+02),
        ];

        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&Params::reference_faithful());
        let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
        let es = maxix * maxiy * maxiz;

        // The initial T-H state `thdiffusion_solverxyz` builds.
        let maxir = params.fuel.maxir;
        let whichk = &geometry.fuel.whichk;
        let mut surfcount = 0usize;
        for ir in 0..maxir - 1 {
            if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                surfcount += 1;
            }
        }
        let maxid = maxir + surfcount;

        let mut th = th;
        th.fueltempavg = vec![params.fueltempavg; es];
        th.fueltempdoppler = vec![params.fueltempavg; es];
        th.fueltemp = {
            let mut a = Array2::<f64>::zeros(es, maxid);
            for i in 0..es {
                for j in 0..maxid {
                    a.set(i, j, params.fueltempavg);
                }
            }
            a
        };
        th.coolant.temps = vec![params.cooltempavg; es];
        th.coolant.dens = vec![params.cooldenavg; es];
        th.heatflux = vec![0.0; es];

        let (sv, ws, rod) = sigmavalupd3d_handler(
            &params,
            &geometry,
            &sigmavalues,
            &feedback,
            &whichsigma,
            &th,
        )
        .expect("the handler should run");

        eprintln!("material rows: {} (MATLAB: {MATLAB_ROWS})", sv.tot.rows());

        // The rod-fraction map, which the MATLAB writes to rodfrac.csv.
        // MATLAB 2026-08-18: sum 2.347473196666667e2, 270 rodded, 221 full,
        // 49 partial, partial values {0.248508666667, 0.562587666667}.
        let rsum: f64 = rod.frac.iter().sum();
        let rodded = rod.frac.iter().filter(|f| **f != 0.0).count();
        let full = rod.frac.iter().filter(|f| **f >= 1.0).count();
        let mut partial: Vec<f64> = rod
            .frac
            .iter()
            .filter(|f| **f > 0.0 && **f < 1.0)
            .map(|f| (f * 1e12).round() / 1e12)
            .collect();
        partial.sort_by(|a, b| a.partial_cmp(b).unwrap());
        partial.dedup();
        eprintln!("rod frac sum : {rsum:.15e}  (MATLAB 2.347473196666667e2)");
        eprintln!("  rel diff   : {:.3e}", (rsum - 2.347473196666667e2).abs() / 2.347473196666667e2);
        eprintln!("  rodded {rodded} (221 full expected: {full}), partial {}", partial.len());
        eprintln!("  distinct partial values: {partial:?}");
        eprintln!("  stale level carryovers : {}", rod.stale_level_carryovers);

        let mut sums = [0.0f64; 6];
        for ix in 0..maxix {
            for iy in 0..maxiy {
                for iz in 0..maxiz {
                    let m = ws.get(ix, iy, iz);
                    if m == 0 {
                        continue;
                    }
                    let r = m - 1; // 1-based ids, 0-based rows
                    sums[0] += sv.tot.get(r, 0);
                    sums[1] += sv.tot.get(r, 1);
                    sums[2] += sv.f.get(r, 0);
                    sums[3] += sv.f.get(r, 1);
                    sums[4] += sv.s.get(r, 1, 0);
                    sums[5] += sv.s.get(r, 0, 0);
                }
            }
        }

        eprintln!("{:<8} {:>22} {:>22} {:>12}", "quantity", "this port", "MATLAB", "rel. diff");
        let mut worst = 0.0f64;
        let mut worst_name = "";
        for (i, (name, want)) in MATLAB.iter().enumerate() {
            let got = sums[i];
            let rel = (got - want).abs() / want.abs();
            eprintln!("{name:<8} {got:>22.12e} {want:>22.12e} {rel:>12.3e}");
            if rel > worst {
                worst = rel;
                worst_name = name;
            }
        }
        eprintln!();
        eprintln!("worst: {worst_name} at {worst:.3e}");

        assert_eq!(
            sv.tot.rows(),
            MATLAB_ROWS,
            "the material renumbering must agree with the MATLAB"
        );
        assert!(
            worst < 1e-10,
            "the feedback handler disagrees with the MATLAB: {worst_name} differs by {worst:e}"
        );
    }

    /// **X1: does a STABLE nodal-update interval make the two codes agree?**
    ///
    /// # Methodology
    ///
    /// At the default `nodalupd = 6` the inner eigensolve is **unstable** on
    /// this case — a cold static solve gives 3661 here and 837 in the MATLAB,
    /// i.e. both diverge, to different garbage. That is defect N1 on a real
    /// case. An unstable iteration separates two implementations at round-off,
    /// so the coupled trajectories cannot be expected to track each other, and
    /// the observed +2144 pcm may be chaos rather than a translation error.
    ///
    /// At `nodalupd = 20` the static solve is **stable and identical** in both
    /// codes (1.0245087144). If the coupled solves then also agree, X1 is
    /// explained: not a mistranslation, but two codes integrating an unstable
    /// iteration to different attractors — one physical, one with a negative
    /// flux.
    ///
    /// If they still disagree at a stable interval, the chaos explanation is
    /// wrong and something in the coupling really does differ.
    ///
    /// `#[ignore]`d: a coupled solve, several minutes.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// # THIS RESOLVES X1.
    ///
    /// At `nodalupd = 20` the two codes agree to **every digit printed**:
    ///
    /// | | MATLAB | this port |
    /// |---|---|---|
    /// | `k_eff` | 1.0139476080 | **1.0139476080** |
    /// | fuel T max | 1180.1501 | **1180.1501** |
    /// | coolant T max | 605.6450 | **605.6450** |
    /// | heat flux sum | 1.224531e5 | **1.224531e5** |
    /// | pwrdens sum | 8.794949e5 | **8.794949e5** |
    ///
    /// Converged in 16 passes, monotonically: `1.0242, 1.0230, 1.0225, 1.0204,
    /// 1.0185, 1.0171, 1.0160, 1.0152, 1.0147, 1.0144, 1.0142, ...` — the
    /// feedback pulling `k_eff` down exactly as it does in the MATLAB. The
    /// power stays **positive** throughout.
    ///
    /// **So the +2144 pcm was never a translation error.** It was chaos. At
    /// the default `nodalupd = 6` the inner eigensolve is unstable on this case
    /// — defect N1 — and a cold static solve diverges in *both* codes, to 3661
    /// here and 837 in the MATLAB. Two implementations integrating an unstable
    /// iteration separate at round-off and land on different attractors: the
    /// MATLAB's happened to be physical, this port's had a negative flux, which
    /// killed the power and with it the feedback.
    ///
    /// **The practical consequence, and it applies to the reference equally:**
    /// **NEACRP case A2 must not be run at the default nodal-update interval.**
    /// The default is `ceil((17+17+18)/10) = 6`, and at 6 the answer is
    /// meaningless in either code. At 20 both are correct and agree exactly.
    /// The MATLAB's own 1.013943 at the default interval is close to the right
    /// answer by luck, not by construction.
    #[test]
    #[ignore = "X1 resolution; a coupled solve, several minutes"]
    fn x1_does_a_stable_nodal_interval_make_the_codes_agree() {
        let base = Params {
            th_model: crate::types::ThModel::Hem,
            nodalupd: 20,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);

        let out = thdiffusion_solverxyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
        )
        .expect("A2 hem at nodalupd 20 should run");

        let t = &out.th;
        let ftmax = t.fueltempavg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ctmax = t.coolant.temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let qsum: f64 = t.heatflux.iter().sum();
        let psum: f64 = out.pwrdens.iter().sum();

        eprintln!("A2 hem, nodalupd = 20:");
        eprintln!("  k_eff        = {:.10}", out.k_eff);
        eprintln!("  termination  = {:?} after {} passes", out.termination, out.iterations);
        eprintln!("  fuel T max   = {ftmax:.4}");
        eprintln!("  coolant Tmax = {ctmax:.4}");
        eprintln!("  heatflux sum = {qsum:.6e}");
        eprintln!("  pwrdens sum  = {psum:.6e}  (negative means the flux went non-physical)");
        eprintln!();
        eprintln!("  at the default nodalupd = 6 this port gives 1.035684 and the MATLAB 1.013943");
        eprintln!("  per-pass k_eff: {:?}",
            out.k_eff_history.iter().take(12).map(|k| (k * 1e4).round() / 1e4).collect::<Vec<_>>());

        assert!(out.k_eff.is_finite());
    }

    /// **X1: the per-pass trace of the coupled loop on A2.**
    ///
    /// # Methodology
    ///
    /// One T-H pass on A2 is correct, and the converged state is degenerate, so
    /// the collapse happens *between* passes. This prints the full per-pass
    /// history — `k_eff`, heat flux, fuel and coolant maxima — so the pass at
    /// which it happens is visible rather than inferred.
    ///
    /// `#[ignore]`d: a coupled solve, several minutes.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **The fission source goes negative, and that kills the feedback.**
    ///
    /// | pass | `k_eff` | pwrdens sum | heat flux sum | fuel T max | cool T max |
    /// |---|---|---|---|---|---|
    /// | 1 | 87.24 | 1.52e6 | 3.55e4 | 1995.6 | 572.4 |
    /// | 9 | 110.70 | 2.77e5 | 1.09e5 | 1844.1 | 677.3 |
    /// | **10** | 60.88 | **-9.08e6** | 8.40e4 | 1844.0 | 617.9 |
    /// | 13 | 94.45 | -3.02e5 | 3.63e4 | 1845.3 | 641.6 |
    /// | 14 | 1.0307 | -3.03e5 | **1.81e4** | 1224.6 | 624.5 |
    /// | 15 | 1.0324 | -3.04e5 | 9.07e3 | 920.2 | 617.9 |
    /// | 16 | 1.0337 | -3.05e5 | 4.53e3 | **891.19** | 616.1 |
    /// | ... | | | *halving* | *pinned* | *decaying* |
    /// | 27 | 1.035684 | -3.06e5 | 2.21e0 | 891.19 | 559.147 |
    ///
    /// **The mechanism, end to end.** From pass 10 the power density is
    /// **negative**. A negative `pinpowdens` makes the rod-conduction solve
    /// return a profile *below* the coolant temperature; `th_solverxyz` then
    /// clamps it up to the coolant; and the wall heat flux is
    /// `hcoeff * (T_surface - T_coolant)` = **exactly zero**. From pass 14 the
    /// heat flux therefore halves precisely each pass — that is
    /// `(1 - 0.5)*old + 0.5*0` — while the fuel pins at `params.fueltempavg`
    /// and the coolant decays back to its inlet. The loop then "converges"
    /// because nothing is moving any more.
    ///
    /// **Against the MATLAB, per pass** (A2, `hem`, full history captured
    /// 2026-08-18):
    ///
    /// ```text
    /// MATLAB : 452.9, 3278.6, 127.3, 855.6, 74.4, 35742.1, 329.8,
    ///          1.033735, 1.028019, 1.022682, 1.019543, ... 1.013943  (23 passes)
    /// port   : 87.2, 243.9, 659.9, 1572.9, 30.7, 134.3, 63.9, 453.6,
    ///          110.7, 60.9, 404.9, 618.9, 94.5,
    ///          1.030652, 1.032362, 1.033708, ... 1.035684            (27 passes)
    /// ```
    ///
    /// **Both codes go through a wild cold-start phase** — the MATLAB's
    /// `k_eff` reaches 35742 — so that is not the defect. The difference is
    /// what happens after:
    ///
    /// - the MATLAB leaves the wild phase at **pass 8** and then **decreases
    ///   monotonically** from 1.0337 to 1.0139 as the Doppler and density
    ///   feedback take hold;
    /// - this port leaves it at **pass 14** and then **creeps upward** from
    ///   1.0307 to 1.0357 and stalls, because by then its power is negative and
    ///   there is no feedback left to pull `k_eff` down.
    ///
    /// Strikingly, the MATLAB's first settled value (1.033735) is almost
    /// exactly this port's pass-16 value (1.033708). Both codes reach the same
    /// neighbourhood; only one of them keeps going.
    ///
    /// **What this rules in and out.** The eigensolver is not at fault — it is
    /// verified exact statically at every stable nodal-update interval. Neither
    /// code guards negative flux: `fixnegative` is **commented out** at
    /// `sanodaldiffusion_solverxyz.m:204` and this port matches that. So the
    /// question is why this port's trajectory enters a negative mode and the
    /// MATLAB's does not, given identical operators.
    ///
    /// **Next step.** The two trajectories differ from pass 1, so compare what
    /// the coupled driver feeds the eigensolver: the **inexact-inner tolerance
    /// schedule** (`innertol`, derived from the outer residual) and the **warm
    /// start** (the flux handed forward between passes). Those are the only two
    /// things the driver varies, and the static comparison has already
    /// eliminated everything downstream of them.
    #[test]
    #[ignore = "X1 per-pass trace; a coupled solve, several minutes"]
    fn x1_per_pass_trace_of_the_coupled_loop() {
        let base = Params {
            th_model: crate::types::ThModel::Hem,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);
        let inlet = th.coolant.inlettemp;

        let out = thdiffusion_solverxyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
        )
        .expect("A2 hem should run");

        eprintln!("A2 hem, per-pass trace (MATLAB converges in 23 to k_eff = 1.013943):");
        eprintln!("  inlet coolant temperature = {inlet:.4} K");
        eprintln!(
            "{:>5}  {:>14}  {:>14}  {:>14}  {:>12}  {:>12}",
            "pass", "k_eff", "pwrdens sum", "heatflux sum", "fuel T max", "cool T max"
        );
        for (i, snap) in out.th_history.iter().enumerate() {
            let k = out.k_eff_history.get(i + 1).or(out.k_eff_history.last());
            eprintln!(
                "{:>5}  {:>14.6}  {:>14.5e}  {:>14.5e}  {:>12.3}  {:>12.4}",
                i + 1,
                k.copied().unwrap_or(f64::NAN),
                snap.pwrdens_sum,
                snap.heatflux_sum,
                snap.fueltemp_max,
                snap.coolant_max
            );
        }
        eprintln!();
        eprintln!("final k_eff = {:.6}, termination {:?}", out.k_eff, out.termination);
    }

    /// **X1: what does one T-H pass on A2 actually do?**
    ///
    /// # Methodology
    ///
    /// The power fed in comes from a **frozen-nodal** static eigensolve, which
    /// is verified exact against the MATLAB (0.00 pcm), so the input is known
    /// good. One call to [`crate::th_solverxyz`] then reports its own
    /// `RodReport` — how many rods were solved, skipped, rescued or clamped —
    /// alongside the resulting temperatures and heat flux.
    ///
    /// `clamped_low` is the interesting counter: it counts rods whose solved
    /// profile came back **below the coolant temperature** and was lifted to
    /// it. If nearly every fuelled rod is clamped low, the conduction solve is
    /// receiving little or no power, which is the symptom X1 now shows.
    ///
    /// [`crate::neacrpd1`] is run alongside as the control — its `hem` path
    /// demonstrably heats its coolant.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | | A2 (15.5 MPa) | D1 (6.7 MPa) |
    /// |---|---|---|
    /// | pwrdens sum | 9.009e5 | 1.109e6 |
    /// | rods solved / skipped | 2512 / 2690 | 2220 / 1826 |
    /// | clamped low / high | 2512 / 0 | 2220 / 22 |
    /// | **heat flux sum** | **1.195e5** | 6.685e4 |
    /// | heat flux max | 158.18 | 268.55 |
    /// | fuel T max | 1124.77 (initial 891.19) | 2150.46 (initial 650) |
    /// | coolant T max | 560.31 (inlet 559.15) | 549.60 (inlet 547.15) |
    ///
    /// **This reverses the diagnosis, and it is the important result.** A
    /// single T-H pass on A2 is **fine**: heat flux 1.195e5 against the
    /// MATLAB's converged 1.2246e5, fuel heating to 1124.77 K against its
    /// 1180.16 K, coolant rising off the inlet. Nothing here is five orders of
    /// magnitude wrong.
    ///
    /// So the thermal-hydraulics is **correct in isolation**. It is the
    /// **coupled loop** that drives it to the degenerate state reported by
    /// `x1_converged_th_state_against_the_matlab` — heat flux 2.21, coolant
    /// pinned exactly at the inlet, fuel pinned exactly at its initial value.
    ///
    /// The earlier conclusion that "the heat flux is five orders of magnitude
    /// too small" described the *converged* state accurately but attributed it
    /// wrongly: the T-H does not compute a bad heat flux, the loop destroys a
    /// good one.
    ///
    /// **Note `clamped_low` equals `solved` in both cases.** Every rod that is
    /// solved is then clamped up to its coolant temperature somewhere in its
    /// profile — expected, because defect T7 pins the gap node at 1 K, so the
    /// clamp always fires on that node. It is not a signal of trouble here.
    ///
    /// **Next step:** instrument the coupled loop per pass — heat flux and fuel
    /// temperature alongside the `k_eff` history already collected — to find
    /// the pass at which a good T-H state collapses. The prime suspect is the
    /// under-relaxation in `relax_th`, which blends `heatflux`,
    /// `fueltempavg`, `fueltempdoppler` and `coolant.dens` but leaves
    /// `coolant.temps` and the radial `fueltemp` profile untouched — a
    /// combination that can hold the fuel at a stale temperature while the
    /// coolant is recomputed fresh.
    #[test]
    fn x1_what_one_th_pass_does_on_a2_versus_d1() {
        for (name, built) in [
            ("A2 (15.5 MPa)", crate::neacrpa2::neacrpa2(&Params {
                nodalupd: 1_000_000_000,
                th_model: crate::types::ThModel::Hem,
                ..Params::reference_faithful()
            })),
            ("D1 (6.7 MPa)", crate::neacrpd1::neacrpd1(&Params {
                nodalupd: 1_000_000_000,
                th_model: crate::types::ThModel::Hem,
                ..Params::reference_faithful()
            })),
        ] {
            let (params, geometry, th, whichsigma, sigmavalues, feedback) = built;
            let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
            let es = maxix * maxiy * maxiz;

            let maxir = params.fuel.maxir;
            let whichk = &geometry.fuel.whichk;
            let mut sc = 0usize;
            for ir in 0..maxir - 1 {
                if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                    sc += 1;
                }
            }
            let maxid = maxir + sc;
            let mut th = th;
            th.fueltempavg = vec![params.fueltempavg; es];
            th.fueltempdoppler = vec![params.fueltempavg; es];
            th.fueltemp = {
                let mut a = Array2::<f64>::zeros(es, maxid);
                for i in 0..es {
                    for j in 0..maxid {
                        a.set(i, j, params.fueltempavg);
                    }
                }
                a
            };
            th.coolant.temps = vec![params.cooltempavg; es];
            th.coolant.dens = vec![params.cooldenavg; es];
            th.heatflux = vec![0.0; es];

            let (sv, ws, _r) = sigmavalupd3d_handler(
                &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
            )
            .expect("handler");
            let eig = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
                &geometry, &params, &sv, &ws, None, None,
            )
            .expect("eigensolve");

            // pwrdens = fission_source .* Vi, exactly as the drivers form it.
            let g = params.g;
            let pwr: Vec<f64> = (0..g * es)
                .map(|i| eig.fission_source[i] * geometry.vi[i % es])
                .collect();

            let (out, rods) =
                crate::th_solverxyz::th_solverxyz(&params, &geometry, &th, &ws, &pwr);

            let stat = |v: &[f64]| {
                let s: f64 = v.iter().sum();
                let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                (s, hi)
            };
            let (qsum, qmax) = stat(&out.heatflux);
            let (_, ftmax) = stat(&out.fueltempavg);
            let (_, ctmax) = stat(&out.coolant.temps);
            eprintln!("=== {name} ===");
            eprintln!("  pwrdens sum   = {:.6e}", pwr.iter().sum::<f64>());
            eprintln!("  rods: solved {} skipped {} rescued {} clamped_low {} clamped_high {}",
                rods.solved, rods.skipped, rods.rescued, rods.clamped_low, rods.clamped_high);
            eprintln!("  heatflux sum  = {qsum:.6e}   max = {qmax:.6}");
            eprintln!("  fuel T max    = {ftmax:.4}   (initial {:.4})", params.fueltempavg);
            eprintln!("  coolant T max = {ctmax:.4}   (inlet {:.4})", th.coolant.inlettemp);
        }
    }

    /// **X1 sweep: how the static eigenvalue moves with the nodal-update interval.**
    ///
    /// # Methodology
    ///
    /// The frozen-nodal comparison matches the MATLAB to 0.00 pcm, so the
    /// operator, the cross sections and the eigensolve are all correct. The
    /// remaining X1 gap must therefore come from **refreshing** the SA-nodal
    /// correction during the source iteration.
    ///
    /// This sweeps `nodalupd` from frozen down to the default and reports
    /// `k_eff` for each, so the sensitivity is visible and a matching MATLAB
    /// sweep can be run at the same values.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | `nodalupd` | this port | MATLAB |
    /// |---|---|---|
    /// | frozen (1e9) | 1.0230689628 | 1.0230689628 |
    /// | 200 | 1.0244511444 | 1.0244511444 |
    /// | 100 | 1.0245061195 | 1.0245061195 |
    /// | 50 | 1.0245084306 | 1.0245084306 |
    /// | 20 | 1.0245087144 | 1.0245087144 |
    /// | **6** (the default) | **3661.34** | **837.30** |
    ///
    /// **Identical at every interval where the solve is stable**, to ten
    /// significant figures. The static neutronics is therefore faithful in
    /// full — not only the operator but the nodal refresh as well.
    ///
    /// **At the default interval of 6 both codes diverge**, to different
    /// garbage, which is what a chaotic divergence does: the trajectories
    /// separate at round-off and never re-converge. That is defect N1
    /// showing up on a real case, and it is the reference's own documented
    /// behaviour for this heavily-rodded configuration.
    ///
    /// Note the converged values also show the nodal refresh is worth
    /// ~140 pcm here (1.02307 frozen against 1.02451 refreshed), and that
    /// it has essentially converged by an interval of 50.
    #[test]
    fn x1_sweep_the_nodal_update_interval() {
        let (maxix, maxiy, maxiz) = (17usize, 17, 18);
        let es = maxix * maxiy * maxiz;

        eprintln!("{:>12}  {:>16}  {:>12}  {:>10}", "nodalupd", "k_eff", "residual", "iters");
        for nodalupd in [1_000_000_000usize, 200, 100, 50, 20, 6] {
            let base = Params { nodalupd, ..Params::reference_faithful() };
            let (params, geometry, th, whichsigma, sigmavalues, feedback) =
                crate::neacrpa2::neacrpa2(&base);

            let maxir = params.fuel.maxir;
            let whichk = &geometry.fuel.whichk;
            let mut surfcount = 0usize;
            for ir in 0..maxir - 1 {
                if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                    surfcount += 1;
                }
            }
            let maxid = maxir + surfcount;
            let mut th = th;
            th.fueltempavg = vec![params.fueltempavg; es];
            th.fueltempdoppler = vec![params.fueltempavg; es];
            th.fueltemp = {
                let mut a = Array2::<f64>::zeros(es, maxid);
                for i in 0..es {
                    for j in 0..maxid {
                        a.set(i, j, params.fueltempavg);
                    }
                }
                a
            };
            th.coolant.temps = vec![params.cooltempavg; es];
            th.coolant.dens = vec![params.cooldenavg; es];
            th.heatflux = vec![0.0; es];

            let (sv, ws, _r) = sigmavalupd3d_handler(
                &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
            )
            .expect("handler");
            match crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
                &geometry, &params, &sv, &ws, None, None,
            ) {
                Ok(o) => eprintln!(
                    "{nodalupd:>12}  {:>16.10}  {:>12.4e}  {:>10}",
                    o.k_eff, o.residual, o.iterations
                ),
                Err(e) => eprintln!("{nodalupd:>12}  ERROR: {e}"),
            }
        }
        eprintln!();
        eprintln!("frozen (1e9) matches the MATLAB exactly at 1.0230689628.");
    }

    /// **X1 bisection: the static eigenvalue with the nodal correction frozen.**
    ///
    /// # Methodology
    ///
    /// This isolates the neutronics from the thermal-hydraulics and from the
    /// SA-nodal *update* schedule, leaving only: the operator assembly, the
    /// one flat-flux nodal build, and the eigenvalue iteration.
    ///
    /// `nodalupd` is set to 1e9 so the correction is built once from the
    /// initial flat flux and never refreshed. That is deterministic and
    /// identical in both codes — and it is also what
    /// `criticalboron_xyz.m`'s own comment says stabilises a cold start here,
    /// where the *continuous* updates diverge (a plain cold eigensolve gives
    /// `k_eff = 837.30` in the MATLAB).
    ///
    /// The cross sections going in are already known to agree to 3.3e-13
    /// (see `x1_does_the_feedback_handler_match_the_matlab`), so any
    /// disagreement here is in the operator or the eigensolve.
    ///
    /// MATLAB R2026a, 2026-08-18 (`x1_frozen.m`):
    ///
    /// ```text
    /// keff     = 1.0230689628
    /// residual = 9.507775e-07
    /// flux sum = 1.1819094459e+04    max = 1.2137257519e+01
    /// ```
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **Exact agreement.**
    ///
    /// | | this port | MATLAB |
    /// |---|---|---|
    /// | `k_eff` | 1.0230689628 | 1.0230689628 (**+0.00 pcm**) |
    /// | residual | 9.507775e-7 | 9.507775e-07 |
    /// | flux sum | 1.1819094459e4 | 1.1819094459e4 (rel 2.9e-11) |
    /// | flux max | 1.2137257519e1 | 1.2137257519e1 |
    ///
    /// **Interpretation.** With the nodal correction frozen, the two codes
    /// agree to ten significant figures on a 17x17x18 two-group core with
    /// all five feedback channels live. That verifies the operator
    /// assembly, the boundary conditions, the flat-flux nodal build, the
    /// eigenvalue iteration and the cross sections **together**, against
    /// the reference rather than against an analytic limit.
    ///
    /// It also localises what remains of X1: whatever differs must involve
    /// **refreshing** the nodal correction, or lie downstream in the
    /// thermal-hydraulics.
    #[test]
    fn x1_frozen_nodal_static_eigenvalue_against_the_matlab() {
        const MATLAB_K_EFF: f64 = 1.0230689628;
        const MATLAB_FLUX_SUM: f64 = 1.1819094459e4;
        const MATLAB_FLUX_MAX: f64 = 1.2137257519e1;

        let base = Params {
            nodalupd: 1_000_000_000,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);
        let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
        let es = maxix * maxiy * maxiz;

        let maxir = params.fuel.maxir;
        let whichk = &geometry.fuel.whichk;
        let mut surfcount = 0usize;
        for ir in 0..maxir - 1 {
            if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                surfcount += 1;
            }
        }
        let maxid = maxir + surfcount;

        let mut th = th;
        th.fueltempavg = vec![params.fueltempavg; es];
        th.fueltempdoppler = vec![params.fueltempavg; es];
        th.fueltemp = {
            let mut a = Array2::<f64>::zeros(es, maxid);
            for i in 0..es {
                for j in 0..maxid {
                    a.set(i, j, params.fueltempavg);
                }
            }
            a
        };
        th.coolant.temps = vec![params.cooltempavg; es];
        th.coolant.dens = vec![params.cooldenavg; es];
        th.heatflux = vec![0.0; es];

        let (sv, ws, _rod) = sigmavalupd3d_handler(
            &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
        )
        .expect("handler");

        let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
            &geometry, &params, &sv, &ws, None, None,
        )
        .expect("the frozen-nodal eigensolve should run");

        let flux: Vec<f64> = (0..out.scalar_flux.rows())
            .map(|i| out.scalar_flux.get(i, 0))
            .collect();
        let fsum: f64 = flux.iter().sum();
        let fmax = flux.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let fmin = flux.iter().cloned().fold(f64::INFINITY, f64::min);

        let pcm = (out.k_eff - MATLAB_K_EFF) / MATLAB_K_EFF * 1e5;
        eprintln!("A2 frozen-nodal static eigenvalue:");
        eprintln!("  this port k_eff = {:.10}", out.k_eff);
        eprintln!("  MATLAB    k_eff = {MATLAB_K_EFF:.10}");
        eprintln!("  difference      = {pcm:+.2} pcm");
        eprintln!("  termination     = {:?} in {} iterations", out.termination, out.iterations);
        eprintln!("  residual        = {:.6e} (MATLAB 9.507775e-07)", out.residual);
        eprintln!("  flux sum        = {fsum:.10e} (MATLAB {MATLAB_FLUX_SUM:.10e})");
        eprintln!("  flux max        = {fmax:.10e} (MATLAB {MATLAB_FLUX_MAX:.10e})");
        eprintln!("  flux min        = {fmin:.10e}");
        eprintln!(
            "  flux sum rel diff = {:.3e}",
            (fsum - MATLAB_FLUX_SUM).abs() / MATLAB_FLUX_SUM
        );

        assert!(
            pcm.abs() < 10.0,
            "frozen-nodal k_eff = {:.10} is {pcm:+.2} pcm from the MATLAB",
            out.k_eff
        );
    }

    /// **X1 bisection: does the converged T-H state match the MATLAB?**
    ///
    /// # Methodology
    ///
    /// Both codes now converge on A2's `hem` path but report `k_eff` 2144 pcm
    /// apart. This asks *where* they part: it runs the same coupled solve and
    /// compares the converged thermal-hydraulic state — fuel temperature,
    /// coolant temperature and density, wall heat flux, total power.
    ///
    /// The logic. If the T-H state matches and only `k_eff` differs, the gap is
    /// **neutronic** — the same temperatures are producing a different
    /// eigenvalue. If the T-H state differs too, the gap is in the
    /// thermal-hydraulics, and the neutronics is only reflecting it.
    ///
    /// MATLAB R2026a, 2026-08-18 (`x1_thstate.m`):
    ///
    /// ```text
    /// keff              = 1.0139434818
    /// fueltempavg  sum  = 4.3246799419e+06   min 575.2943   max 1180.1565
    /// coolant.temps sum = 2.9742608313e+06   min 559.1472   max 605.6472
    /// coolant.dens  sum = 3.7763913502e+03   min 0.643442   max 0.753612
    /// heatflux      sum = 1.2245520242e+05   max 142.328117
    /// pwrdens       sum = 8.7949291592e+05
    /// ```
    ///
    /// `#[ignore]`d: a coupled solve, several minutes.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **The thermal-hydraulics is where the remaining gap lives.**
    ///
    /// | | this port | MATLAB | rel. |
    /// |---|---|---|---|
    /// | `k_eff` | 1.0356836577 | 1.0139434818 | +2144.1 pcm |
    /// | fuel T sum | 3.8007e6 | 4.3247e6 | 1.2e-1 |
    /// | fuel T min/max | 558.10 / **891.19** | 575.29 / **1180.16** | |
    /// | coolant T min/max | 558.10 / **559.15** | 559.15 / **605.65** | |
    /// | coolant density | 3.9226e3 | 3.7764e3 | 3.9e-2 |
    /// | **heat flux sum** | **2.21** | **1.2246e5** | **1.000** |
    /// | heat flux max | 0.2806 | 142.33 | |
    /// | power density sum | 9.0194e5 | 8.7949e5 | 2.6e-2 |
    ///
    /// **Interpretation.** The power is right to 2.6%, but the heat flux
    /// derived from it is **five orders of magnitude too small**, so
    /// essentially no heat reaches the coolant. The consequences are
    /// visible in the temperatures: this port's coolant maximum is
    /// **559.15 K, exactly the inlet** — it never heats — and its fuel
    /// maximum is **891.19 K, exactly `params.fueltempavg`**, the initial
    /// value that unfuelled nodes retain. The fuelled nodes have collapsed
    /// to coolant temperature instead of heating to the MATLAB's 1180 K.
    ///
    /// Combined with the neutronics being verified exact, **the whole of
    /// the remaining X1 gap is in the T-H chain**, and specifically in
    /// whatever produces `heatflux`.
    ///
    /// `heatflux = hcoeff * (fueltemp(:, end) - temps)`, and `hcoeff`
    /// derives from a Nusselt correlation in the mixture velocity
    /// `th.coolant.vm`. Checked and **not** the cause: `vm` is populated by
    /// [`crate::singleflow1devap`] and read after it in the right order,
    /// and the IF97 transport properties classify A2's 15.5 MPa / 560 K
    /// state as region 1 correctly. The next candidates are the Nusselt
    /// chain itself (`kvis`, `pran`, `tcon` units) and the rod-surface
    /// temperature `fueltemp(:, end)`.
    ///
    /// **Closed 2026-08-19.** "Why D1 works and A2 does not" had a single
    /// answer, and it was not in the Nusselt chain this note was working
    /// towards: **defect Z1**, the silently rounded axial mesh. A2 grades its
    /// mesh and so was meshing 428 cm against a `Ztot` of 427.3, while D1's
    /// uniform 30 cm layers made it far less sensitive. With Z1 reproduced —
    /// and `nodalupd = 20` for defect N1 — A2 matches the MATLAB exactly. The
    /// candidate list above (`kvis`, `pran`, `tcon` units, the rod-surface
    /// temperature) was **not** the cause and needs no further pursuit.
    #[test]
    #[ignore = "X1 bisection; a coupled solve, several minutes"]
    fn x1_converged_th_state_against_the_matlab() {
        const M_KEFF: f64 = 1.0139434818;
        const M_FUEL_SUM: f64 = 4.3246799419e6;
        const M_FUEL_MIN: f64 = 575.2943;
        const M_FUEL_MAX: f64 = 1180.1565;
        const M_COOL_SUM: f64 = 2.9742608313e6;
        const M_COOL_MAX: f64 = 605.6472;
        const M_DENS_SUM: f64 = 3.7763913502e3;
        const M_FLUX_SUM: f64 = 1.2245520242e5;
        const M_PWR_SUM: f64 = 8.7949291592e5;

        let base = Params {
            th_model: crate::types::ThModel::Hem,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);

        let out = thdiffusion_solverxyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
        )
        .expect("A2 hem should run");

        let t = &out.th;
        let stat = |v: &[f64]| {
            let s: f64 = v.iter().sum();
            let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (s, lo, hi)
        };
        let (fsum, fmin, fmax) = stat(&t.fueltempavg);
        let (csum, cmin, cmax) = stat(&t.coolant.temps);
        let (dsum, _, _) = stat(&t.coolant.dens);
        let (qsum, _, qmax) = stat(&t.heatflux);
        let psum: f64 = out.pwrdens.iter().sum();

        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
        eprintln!("A2 converged T-H state, this port vs MATLAB:");
        eprintln!("  k_eff        {:.10}  vs {M_KEFF:.10}   ({:+.1} pcm)",
            out.k_eff, (out.k_eff - M_KEFF) / M_KEFF * 1e5);
        eprintln!("  fuel T sum   {fsum:.10e}  vs {M_FUEL_SUM:.10e}   rel {:.3e}", rel(fsum, M_FUEL_SUM));
        eprintln!("    min/max    {fmin:.4}/{fmax:.4}  vs {M_FUEL_MIN:.4}/{M_FUEL_MAX:.4}");
        eprintln!("  cool T sum   {csum:.10e}  vs {M_COOL_SUM:.10e}   rel {:.3e}", rel(csum, M_COOL_SUM));
        eprintln!("    min/max    {cmin:.4}/{cmax:.4}  vs 559.1472/{M_COOL_MAX:.4}");
        eprintln!("  cool dens    {dsum:.10e}  vs {M_DENS_SUM:.10e}   rel {:.3e}", rel(dsum, M_DENS_SUM));
        eprintln!("  heat flux    {qsum:.10e}  vs {M_FLUX_SUM:.10e}   rel {:.3e}", rel(qsum, M_FLUX_SUM));
        eprintln!("    max        {qmax:.6}  vs 142.328117");
        eprintln!("  pwrdens sum  {psum:.10e}  vs {M_PWR_SUM:.10e}   rel {:.3e}", rel(psum, M_PWR_SUM));
        eprintln!();
        eprintln!("If the T-H matches and only k_eff differs, the gap is neutronic.");

        assert!(out.k_eff.is_finite());
    }

    /// **MATLAB parity on the FULL 20 s transient — D1 as specified.**
    ///
    /// # Methodology
    ///
    /// The earlier transient comparisons ran 0.15 to 0.5 s. This runs
    /// [`crate::neacrpd1t`] over its **own specified 20 s window on its own
    /// refined grid** — 261 points, from 25 ms steps early to 200 ms late —
    /// closing the "short window only" gap.
    ///
    /// A long window is a different test from a short one: errors that are
    /// invisible over 50 steps have 261 to accumulate in, and the power here is
    /// not monotonic — it rises to a peak of 1.567 at `t = 1.775 s` and then
    /// decays to 1.193, so the comparison covers a turning point rather than a
    /// single trend.
    ///
    /// It also exercises the **fuel melting clamp**: the MATLAB's final maximum
    /// fuel temperature is exactly 3100.000000 K, which is `tmaxfuel`, so the
    /// clamp is active and both codes must hit it identically.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_d1t_full.m`):
    ///
    /// ```text
    /// steps          = 261
    /// P/P0 final     = 1.1928739085
    /// P/P0 max       = 1.5668303538 at t = 1.775000
    /// avg fuel T     = 829.977778
    /// max fuel T     = 3100.000000   (== tmaxfuel, clamped)
    /// coolant outlet = 549.186387
    /// precursor sum  = 1.6370078148e+01
    ///
    /// C1: t= 0.00 1.0000000000   t= 1.25 1.3843727790   t= 3.00 1.4444569395
    ///     t= 5.50 1.3117762159   t=10.00 1.2129701232   t=20.00 1.1928739085
    /// ```
    ///
    /// `#[ignore]`d: 260 transient steps on top of a coupled steady solve.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Exact over the full specified window.**
    ///
    /// | quantity | agreement |
    /// |---|---|
    /// | steps | 261 = 261 |
    /// | `P/P0` final | **2.9e-11** |
    /// | `P/P0` max, and its time | 1.5668303538 at 1.775000, both |
    /// | avg / max fuel T, coolant outlet | identical to every printed digit |
    /// | max fuel T | **3100.000000** both — the `tmaxfuel` clamp |
    /// | precursor sum | 1.2e-11 |
    /// | C1 history, 6 points across 20 s | worst **3.6e-11** |
    ///
    /// **Interpretation.** 261 steps of accumulation leave the agreement at
    /// the same 1e-11 level as the 51-step run, so nothing drifts with
    /// window length. The power trace is **non-monotonic** — rising to 1.567
    /// at 1.775 s then decaying to 1.193 — and both codes place the turning
    /// point at the same time step, which a monotonic trace could not have
    /// tested.
    ///
    /// The **fuel melting clamp is active**: both reach exactly 3100 K, so
    /// this run also verifies the clamp fires identically rather than one
    /// code saturating a step earlier than the other.
    #[test]
    #[ignore = "MATLAB parity on the full 20 s transient; many minutes"]
    fn matlab_parity_neacrpd1t_full_twenty_seconds() {
        const M_PREL_FINAL: f64 = 1.1928739085;
        const M_PREL_MAX: f64 = 1.5668303538;
        const M_TPMAX: f64 = 1.775000;
        const M_AVGFUEL: f64 = 829.977778;
        const M_MAXFUEL: f64 = 3100.000000;
        const M_COOLOUT: f64 = 549.186387;
        const M_PRECURSOR: f64 = 1.6370078148e1;
        // (index, time, P/P0)
        const M_C1: [(usize, f64, f64); 6] = [
            (0, 0.00, 1.0000000000),
            (50, 1.25, 1.3843727790),
            (100, 3.00, 1.4444569395),
            (150, 5.50, 1.3117762159),
            (200, 10.00, 1.2129701232),
            (260, 20.00, 1.1928739085),
        ];

        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpd1t::neacrpd1t(&Params::reference_faithful());

        let out = crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, None, None,
        )
        .expect("the full D1 transient should run");

        let n = out.time.len() - 1;
        let psum: f64 = (0..out.precursors_final.rows())
            .flat_map(|r| (0..out.precursors_final.cols()).map(move |c| (r, c)))
            .map(|(r, c)| out.precursors_final.get(r, c))
            .sum();
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();

        eprintln!("D1 FULL 20 s transient — this port vs MATLAB:");
        eprintln!("  steps          {}  vs 261", out.time.len());
        eprintln!("  P/P0 final     {:.10}  vs {M_PREL_FINAL:.10}  rel {:.3e}",
            out.relpower[n], rel(out.relpower[n], M_PREL_FINAL));
        eprintln!("  P/P0 max       {:.10} at t={:.6}  vs {M_PREL_MAX:.10} at {M_TPMAX:.6}",
            out.prelmax, out.tpmax);
        eprintln!("  avg fuel T     {:.6}  vs {M_AVGFUEL:.6}", out.avgfueltemp[n]);
        eprintln!("  max fuel T     {:.6}  vs {M_MAXFUEL:.6}  (tmaxfuel clamp)",
            out.maxfueltemp[n]);
        eprintln!("  coolant outlet {:.6}  vs {M_COOLOUT:.6}", out.coolouttemp[n]);
        eprintln!("  precursor sum  {psum:.10e}  vs {M_PRECURSOR:.10e}  rel {:.3e}",
            rel(psum, M_PRECURSOR));
        eprintln!("  --- C1 history ---");
        let mut worst = 0.0f64;
        for (i, t, want) in M_C1 {
            if i < out.relpower.len() {
                let d = rel(out.relpower[i], want);
                worst = worst.max(d);
                eprintln!("    t={t:6.2}  {:.10} vs {want:.10}  rel {d:.3e}", out.relpower[i]);
            }
        }
        eprintln!("  worst C1 relative difference: {worst:.3e}");

        assert_eq!(out.time.len(), 261, "both codes must march the same 261-point grid");
        assert!((out.tpmax - M_TPMAX).abs() < 1e-9, "the peak must occur at the same time");
        assert!(
            worst < 1e-4,
            "the 20 s C1 history differs from the MATLAB by {worst:e}"
        );
    }

    /// **MATLAB parity on IAEA-3D.**
    ///
    /// # Methodology
    ///
    /// [`crate::iaea3ds`] has been compared to the *published* PARCS and ADPRES
    /// eigenvalues since it landed, but never to the MATLAB it was translated
    /// from. Those are different claims: matching a published number says the
    /// physics is right, matching the reference says the *translation* is. This
    /// closes that loop.
    ///
    /// It also covers ground the NEACRP comparisons do not: IAEA-3D is pure
    /// neutronics — no thermal-hydraulics, no feedback, no rods — on a uniform
    /// 10 cm / 20 cm mesh with five materials.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_iaea.m`), at the case's own default
    /// `nodalupd = 6`:
    ///
    /// ```text
    /// keff        = 1.0290842762
    /// residual    = 9.611040e-07
    /// flux sum    = 1.7067179422e+04
    /// flux max    = 9.9113901351e+00
    /// pwrdens sum = 8.1243000000e+05
    /// ```
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Exact.** `k_eff = 1.0290842762` in both, **-0.0000 pcm**; flux sum
    /// 1.4e-11, flux max 1.0e-12, power density 4.0e-14.
    ///
    /// **Interpretation.** This closes a loop that had been open since the
    /// case landed. IAEA-3D matched the *published* PARCS and ADPRES values
    /// to -1.1 and +0.2 pcm, but had never been checked against the code it
    /// was translated from — and those are different claims. Matching a
    /// published number says the physics is right; matching the reference
    /// says the translation is. Both now hold.
    ///
    /// It also covers ground the NEACRP comparisons cannot: pure
    /// neutronics, no thermal-hydraulics, no feedback, no rods, on a uniform
    /// mesh — and at the case's own default nodal-update interval, which
    /// unlike A2's is stable.
    #[test]
    fn matlab_parity_iaea3ds() {
        const M_KEFF: f64 = 1.0290842762;
        const M_FLUXSUM: f64 = 1.7067179422e4;
        const M_FLUXMAX: f64 = 9.9113901351;
        const M_PWRSUM: f64 = 8.1243000000e5;

        let params = Params { nodalupd: 6, ..Params::reference_faithful() };
        let (params, geometry, whichsigma, sigmavalues) = crate::iaea3ds::iaea3ds(&params);
        let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .expect("IAEA-3D should solve");

        let flux: Vec<f64> = (0..out.scalar_flux.rows())
            .map(|i| out.scalar_flux.get(i, 0))
            .collect();
        let fsum: f64 = flux.iter().sum();
        let fmax = flux.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let psum: f64 = out.pwrdens.iter().sum();
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
        let pcm = (out.k_eff - M_KEFF) / M_KEFF * 1e5;

        eprintln!("IAEA-3D, this port vs MATLAB:");
        eprintln!("  k_eff       {:.10}  vs {M_KEFF:.10}  ({pcm:+.4} pcm)", out.k_eff);
        eprintln!("  residual    {:.6e}  vs 9.611040e-07", out.residual);
        eprintln!("  flux sum    {fsum:.10e}  vs {M_FLUXSUM:.10e}  rel {:.3e}", rel(fsum, M_FLUXSUM));
        eprintln!("  flux max    {fmax:.10e}  vs {M_FLUXMAX:.10e}  rel {:.3e}", rel(fmax, M_FLUXMAX));
        eprintln!("  pwrdens sum {psum:.10e}  vs {M_PWRSUM:.10e}  rel {:.3e}", rel(psum, M_PWRSUM));
        eprintln!();
        eprintln!("  published: PARCS {:.6}, ADPRES {:.6}",
            crate::iaea3ds::REFERENCE_K_EFF_PARCS, crate::iaea3ds::REFERENCE_K_EFF_ADPRES);

        assert!(pcm.abs() < 0.01, "IAEA-3D k_eff is {pcm:+.4} pcm from the MATLAB");
        assert!(rel(fsum, M_FLUXSUM) < 1e-9);
    }

    /// **The critical-boron search fails on A2 in the MATLAB too.**
    ///
    /// # Methodology
    ///
    /// The last uncompared component. Run `criticalboron_xyz` on A2 with
    /// `th_model = hem` and `nodalupd = 20` — the settings under which every
    /// other A2 comparison agrees exactly — in both codes.
    ///
    /// **The MATLAB aborts.** Its own sanity guard trips:
    ///
    /// ```text
    /// Error using criticalboron_xyz (line 223)
    /// eigenvalue out of sane range (keff = 4.71097 at 1139.19 ppm) - aborting search
    /// ```
    ///
    /// and the iterations just before it read `Keff = 1.000000`, then
    /// `0.999997`, then `4.710974` — so the search had essentially found the
    /// critical point (1139.19 ppm, against the reference's own quoted 1139.01)
    /// and was then thrown off by a single destabilised eigensolve.
    ///
    /// That is defect **N1** again, inside the search's Phase 2 this time: even
    /// at `nodalupd = 20` the warm-started eigensolve can wander, and the guard
    /// added in the June 2026 rewrite catches it and stops rather than letting
    /// it poison the secant.
    ///
    /// So the expected parity result is **both codes failing the same way**,
    /// not both converging. This test asserts that this port also refuses
    /// rather than returning a number the MATLAB would not stand behind.
    ///
    /// `#[ignore]`d: a coupled solve plus a search.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Both codes abort, at the same boron concentration.**
    ///
    /// | | boron at abort | `k_eff` reported |
    /// |---|---|---|
    /// | MATLAB | 1139.19 ppm | 4.71097 |
    /// | this port | 1139.1939954 ppm | 50.664 |
    ///
    /// **Interpretation.** The two searches track each other exactly up to
    /// the point where one eigensolve destabilises, and both sanity guards
    /// then fire at the same concentration. The reported `k_eff` values
    /// differ because they are chaotically diverged garbage — the same
    /// signature seen elsewhere under defect N1, where two implementations
    /// integrating an unstable iteration produce different nonsense.
    ///
    /// This is parity in the strongest form available for an unstable
    /// regime: **the port fails where the reference fails, and in the same
    /// place.** Both had essentially reached the reference's own quoted
    /// 1139.01 ppm before being thrown off.
    ///
    /// It also vindicates the guard the June 2026 rewrite added: it is doing
    /// exactly its job, refusing rather than letting a garbage eigenvalue
    /// poison the secant — the failure mode that once sent boron past
    /// 1e5 ppm.
    #[test]
    #[ignore = "MATLAB parity on the boron search; several minutes"]
    fn matlab_parity_criticalboron_on_a2() {
        let base = Params {
            th_model: crate::types::ThModel::Hem,
            nodalupd: 20,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);

        let r = criticalboron_xyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, None, None,
        );

        eprintln!("criticalboron_xyz on A2 (hem, nodalupd 20):");
        eprintln!("  MATLAB: ABORTS — 'eigenvalue out of sane range");
        eprintln!("          (keff = 4.71097 at 1139.19 ppm) - aborting search'");
        match &r {
            Ok(o) => {
                eprintln!("  this port: converged to {:.4} ppm, k_eff {:.6}", o.boron, o.k_eff);
                eprintln!("             slope {:.2} pcm/ppm, bootstrapped {}",
                    o.slope_pcm_per_ppm, o.bootstrapped);
                eprintln!("             boron history: {:?}",
                    o.boron_history.iter().map(|b| (b * 100.0).round() / 100.0)
                        .collect::<Vec<_>>());
            }
            Err(e) => eprintln!("  this port: ABORTS — {e}"),
        }
        eprintln!();
        eprintln!("  the reference's own quoted critical boron is {:.2} ppm",
            crate::neacrpa2t::CRITICAL_BORON);

        // Whatever happens, it must not silently return a wrong number: either
        // it refuses like the MATLAB, or it lands near the reference's value.
        match r {
            Err(crate::error::BedokError::BoronSearchDiverged { k_eff, boron, .. }) => {
                eprintln!("  -> matches the MATLAB's behaviour (guard tripped)");
                assert!(!(0.8..=1.2).contains(&k_eff));
                assert!(boron > 0.0);
            }
            Err(e) => panic!("unexpected error: {e}"),
            Ok(o) => {
                let d = (o.boron - crate::neacrpa2t::CRITICAL_BORON).abs();
                eprintln!("  -> converged; {d:.2} ppm from the reference's quoted value");
                assert!(
                    d < 50.0,
                    "if it converges where the MATLAB aborts, it must at least land \
                     near the reference's own 1139.01 ppm; got {:.2}",
                    o.boron
                );
            }
        }
    }

    /// **MATLAB parity on the SUPER-PROMPT ejection — NEACRP A1 at HZP.**
    ///
    /// # Methodology
    ///
    /// The hardest transient in the snapshot and the strongest test of the
    /// kinetics. At hot zero power the fuel starts in equilibrium with the
    /// coolant, so there is **no stored Doppler margin**, and bank 1 is pulled
    /// from **fully inserted** to fully withdrawn in 0.1 s. The power grows by a
    /// factor of **67 in 0.15 s** and is still rising.
    ///
    /// That exponential growth is what makes this a stress test: over 151 steps
    /// any difference between the two codes compounds. A discrepancy that would
    /// be invisible in a steady solve is amplified enormously here.
    ///
    /// It uses the **case's own 1 ms grid**, truncated to 0.15 s, rather than
    /// the generic 10 ms one — the reference's comment says the spike needs it.
    /// `nodalupd = 20` for the same reason as A2.
    ///
    /// This is also the regime
    /// [`crate::thdiffusion_solvertimexyz`]'s `freqmode` note is about:
    /// per-node exponential-transform frequencies are unstable in super-prompt
    /// ejections, so the global mode is the default and this run exercises it.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_a1t.m`):
    ///
    /// ```text
    /// steady keff       = 0.9999895928
    /// re-equilibrated   = 0.9999984297
    /// steps marched     = 151
    /// C1 P/P0 final     = 6.7347631101e+01   (max, at t = 0.15)
    /// C2 avg fuel T     = 559.147421 -> 559.147667
    /// C3 max fuel T     = 559.148391 -> 559.149608
    /// precursor sum     = 2.8530751341e+01
    ///
    /// C1: t=0.000 rod=0.0    1.0000000000e+00
    ///     t=0.025 rod=57.0   1.0766608849e+00
    ///     t=0.050 rod=114.0  1.8417478668e+00
    ///     t=0.075 rod=171.0  5.2886201774e+00
    ///     t=0.100 rod=228.0  1.5361374500e+01
    ///     t=0.125 rod=228.0  3.4192790557e+01
    ///     t=0.150 rod=228.0  6.7347631101e+01
    /// ```
    ///
    /// `#[ignore]`d: a coupled steady solve plus 150 transient steps.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Agreement to 2.1e-7 through a 67-fold power excursion.**
    ///
    /// | quantity | agreement |
    /// |---|---|
    /// | steady / re-equilibrated `k_eff` | +0.00 / -0.00 pcm |
    /// | steps marched | 151 = 151 |
    /// | C1 `P/P0` final (67.3x growth) | **2.1e-7** |
    /// | C2 / C3 max fuel T | identical to every printed digit |
    /// | precursor sum | 2.8e-8 |
    /// | C1 history, 7 points | worst **2.1e-7** |
    ///
    /// **Interpretation — and the growth pattern is the point.** The
    /// relative difference starts at 4.4e-11 and grows to 2.1e-7 as the
    /// power amplifies 67-fold. That is the signature of **round-off being
    /// compounded by exponential growth**, not of a real difference: the
    /// per-step discrepancy stays at the 1e-9 level throughout, and the
    /// running total simply inherits the amplification.
    ///
    /// This is the strongest transient evidence available. A super-prompt
    /// excursion is the most unforgiving test of a kinetics implementation —
    /// any systematic error in the exponential transform, the frequency
    /// clamp, or the analytic precursor integration would be magnified by
    /// the same factor the power is, and would show up as a divergence over
    /// 151 steps rather than as parts in ten million.
    #[test]
    #[ignore = "MATLAB parity on the super-prompt ejection; several minutes"]
    fn matlab_parity_neacrpa1t_super_prompt() {
        const M_STEADY: f64 = 0.9999895928;
        const M_REEQ: f64 = 0.9999984297;
        const M_PREL_FINAL: f64 = 6.7347631101e1;
        const M_AVGFUEL_N: f64 = 559.147667;
        const M_MAXFUEL_N: f64 = 559.149608;
        const M_PRECURSOR: f64 = 2.8530751341e1;
        // every 25th step: (rod position, P/P0)
        const M_C1: [(f64, f64); 7] = [
            (0.0, 1.0000000000e0),
            (57.0, 1.0766608849e0),
            (114.0, 1.8417478668e0),
            (171.0, 5.2886201774e0),
            (228.0, 1.5361374500e1),
            (228.0, 3.4192790557e1),
            (228.0, 6.7347631101e1),
        ];

        let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa1t::neacrpa1t(&Params::reference_faithful());
        params.th_model = crate::types::ThModel::Hem;
        params.nodalupd = 20;
        params.tend = Some(0.15); // keeps the case's own 1 ms grid, truncated

        let out = crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, None, None,
        )
        .expect("the A1 super-prompt transient should run");

        let n = out.time.len() - 1;
        let psum: f64 = (0..out.precursors_final.rows())
            .flat_map(|r| (0..out.precursors_final.cols()).map(move |c| (r, c)))
            .map(|(r, c)| out.precursors_final.get(r, c))
            .sum();
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();

        eprintln!("A1 HZP super-prompt ejection, 0 to 0.15 s:");
        eprintln!("  steady k_eff     {:.10}  vs {M_STEADY:.10}  ({:+.2} pcm)",
            out.steady.k_eff, (out.steady.k_eff - M_STEADY) / M_STEADY * 1e5);
        eprintln!("  re-equilibrated  {:.10}  vs {M_REEQ:.10}  ({:+.2} pcm)",
            out.k_eff, (out.k_eff - M_REEQ) / M_REEQ * 1e5);
        eprintln!("  steps marched    {}  vs 151", out.time.len());
        eprintln!("  C1 P/P0 final    {:.10e}  vs {M_PREL_FINAL:.10e}  rel {:.3e}",
            out.relpower[n], rel(out.relpower[n], M_PREL_FINAL));
        eprintln!("  C2 avg fuel T    -> {:.6}  vs {M_AVGFUEL_N:.6}", out.avgfueltemp[n]);
        eprintln!("  C3 max fuel T    -> {:.6}  vs {M_MAXFUEL_N:.6}", out.maxfueltemp[n]);
        eprintln!("  precursor sum    {psum:.10e}  vs {M_PRECURSOR:.10e}  rel {:.3e}",
            rel(psum, M_PRECURSOR));
        eprintln!("  --- C1 history ---");
        let mut worst = 0.0f64;
        for (k, (rod_want, p_want)) in M_C1.iter().enumerate() {
            let i = k * 25;
            if i < out.relpower.len() {
                let d = rel(out.relpower[i], *p_want);
                worst = worst.max(d);
                eprintln!("    t={:.3}  rod={:7.2} vs {rod_want:7.2}   {:.10e} vs {p_want:.10e}  rel {d:.3e}",
                    out.time[i], out.rodpos[i], out.relpower[i]);
            }
        }
        eprintln!("  worst C1 relative difference: {worst:.3e}");
        eprintln!("  (power grew {:.1}x over the window)", out.relpower[n]);

        assert_eq!(out.time.len(), 151, "both codes must march the same grid");
        assert!(
            out.relpower[n] > 50.0,
            "this is a super-prompt excursion; the power must grow sharply"
        );
        assert!(
            worst < 1e-4,
            "the C1 power history differs from the MATLAB by {worst:e}"
        );
    }

    /// **MATLAB parity on the ROD-EJECTION transient — NEACRP A2.**
    ///
    /// # Methodology
    ///
    /// D1t verified the transient chain on a case with **no rod motion**. This
    /// adds the piece D1t cannot reach: the prescribed control-assembly
    /// ejection, bank 1 driven from 100 to 228 steps over 0.1 s, with the
    /// cross sections rebuilt against a moving rod every step.
    ///
    /// The window is 0 to 0.15 s on the driver's generic 10 ms grid, covering
    /// the whole ejection plus a little after, so both the ramp and the
    /// post-ejection plateau are compared.
    ///
    /// `nodalupd = 20` because A2's inner eigensolve is unstable at the default
    /// of 6 (defect N1), which would make the phase-1 steady state meaningless
    /// in either code.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_a2t.m`):
    ///
    /// ```text
    /// steady keff       = 1.0000261575
    /// re-equilibrated   = 1.0000164271
    /// steps marched     = 16
    /// C1 P/P0 final     = 1.0881240813
    /// C1 P/P0 max       = 1.0894356489 at t = 0.1000
    /// C2 avg fuel T     = 869.209923 -> 870.405100
    /// C3 max fuel T     = 1995.933291 -> 1997.660560
    /// C4 coolant outlet = 598.148142 -> 598.149746
    /// rod position      = 100 -> 228 steps
    /// precursor sum     = 2.4067900880e+01
    ///
    /// C1 history: t=0.00 rod=100.0 1.00000000
    ///             t=0.03 rod=138.4 1.05092788
    ///             t=0.06 rod=176.8 1.08285027
    ///             t=0.09 rod=215.2 1.08939854
    ///             t=0.12 rod=228.0 1.08901976
    ///             t=0.15 rod=228.0 1.08812408
    /// ```
    ///
    /// The steady `k_eff` of 1.0000262 also confirms the reference's own
    /// 1139.01 ppm critical boron really does make this case critical, once a
    /// stable nodal-update interval is used.
    ///
    /// `#[ignore]`d: a coupled steady solve plus 15 transient steps.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Exact agreement, including the moving rod.**
    ///
    /// | quantity | agreement |
    /// |---|---|
    /// | steady / re-equilibrated `k_eff` | +0.00 / +0.00 pcm |
    /// | steps marched | 16 = 16 |
    /// | C1 `P/P0` final | **2.6e-11** |
    /// | C1 `P/P0` max and its time | 1.0894356489 at 0.1000, both |
    /// | C2 / C3 / C4 | identical to every printed digit |
    /// | rod position, every step | **0.000e0** steps |
    /// | precursor sum | 1.6e-11 |
    /// | C1 history, 6 points | worst **3.7e-9** |
    ///
    /// **Interpretation.** The ejection ramp is reproduced exactly — the rod
    /// position agrees to zero at every step, including the clamp once it
    /// reaches 228 — and so is the power response to it, through a
    /// cross-section rebuild against a moving rod on every time step.
    ///
    /// The steady `k_eff` of 1.0000262 also confirms the reference's own
    /// 1139.01 ppm critical boron: at a stable nodal-update interval the
    /// case really is critical to 2.6 pcm.
    #[test]
    #[ignore = "MATLAB parity on the rod-ejection transient; several minutes"]
    fn matlab_parity_neacrpa2t_rod_ejection() {
        const M_STEADY: f64 = 1.0000261575;
        const M_REEQ: f64 = 1.0000164271;
        const M_PREL_FINAL: f64 = 1.0881240813;
        const M_PREL_MAX: f64 = 1.0894356489;
        const M_AVGFUEL_N: f64 = 870.405100;
        const M_MAXFUEL_N: f64 = 1997.660560;
        const M_COOLOUT_N: f64 = 598.149746;
        const M_PRECURSOR: f64 = 2.4067900880e1;
        // t = 0.00, 0.03, 0.06, 0.09, 0.12, 0.15 and the rod position there
        const M_C1: [(f64, f64); 6] = [
            (100.0, 1.00000000),
            (138.4, 1.05092788),
            (176.8, 1.08285027),
            (215.2, 1.08939854),
            (228.0, 1.08901976),
            (228.0, 1.08812408),
        ];

        let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2t::neacrpa2t(&Params::reference_faithful());
        params.th_model = crate::types::ThModel::Hem;
        params.nodalupd = 20;
        params.tend = Some(0.15);
        params.tgrid = None;

        let out = crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
            None,
        )
        .expect("the A2 rod-ejection transient should run");

        let n = out.time.len() - 1;
        let psum: f64 = (0..out.precursors_final.rows())
            .flat_map(|r| (0..out.precursors_final.cols()).map(move |c| (r, c)))
            .map(|(r, c)| out.precursors_final.get(r, c))
            .sum();
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();

        eprintln!("A2 rod ejection, 0 to 0.15 s — this port vs MATLAB:");
        eprintln!("  steady k_eff     {:.10}  vs {M_STEADY:.10}  ({:+.2} pcm)",
            out.steady.k_eff, (out.steady.k_eff - M_STEADY) / M_STEADY * 1e5);
        eprintln!("  re-equilibrated  {:.10}  vs {M_REEQ:.10}  ({:+.2} pcm)",
            out.k_eff, (out.k_eff - M_REEQ) / M_REEQ * 1e5);
        eprintln!("  steps marched    {}  vs 16", out.time.len());
        eprintln!("  C1 P/P0 final    {:.10}  vs {M_PREL_FINAL:.10}  rel {:.3e}",
            out.relpower[n], rel(out.relpower[n], M_PREL_FINAL));
        eprintln!("  C1 P/P0 max      {:.10} at t={:.4}  vs {M_PREL_MAX:.10} at 0.1000",
            out.prelmax, out.tpmax);
        eprintln!("  C2 avg fuel T    -> {:.6}  vs {M_AVGFUEL_N:.6}", out.avgfueltemp[n]);
        eprintln!("  C3 max fuel T    -> {:.6}  vs {M_MAXFUEL_N:.6}", out.maxfueltemp[n]);
        eprintln!("  C4 coolant out   -> {:.6}  vs {M_COOLOUT_N:.6}", out.coolouttemp[n]);
        eprintln!("  rod position     {:.4} -> {:.4}  vs 100 -> 228",
            out.rodpos[0], out.rodpos[n]);
        eprintln!("  precursor sum    {psum:.10e}  vs {M_PRECURSOR:.10e}  rel {:.3e}",
            rel(psum, M_PRECURSOR));
        eprintln!("  --- C1 history (rod, P/P0) ---");
        let mut worst = 0.0f64;
        let mut worst_rod = 0.0f64;
        for (k, (rod_want, p_want)) in M_C1.iter().enumerate() {
            let i = k * 3;
            if i < out.relpower.len() {
                let d = rel(out.relpower[i], *p_want);
                let dr = (out.rodpos[i] - rod_want).abs();
                worst = worst.max(d);
                worst_rod = worst_rod.max(dr);
                eprintln!("    t={:.3}  rod={:7.2} vs {rod_want:7.2}   {:.8} vs {p_want:.8}  rel {d:.3e}",
                    out.time[i], out.rodpos[i], out.relpower[i]);
            }
        }
        eprintln!("  worst C1 relative difference: {worst:.3e}");
        eprintln!("  worst rod-position difference: {worst_rod:.3e} steps");

        assert_eq!(out.time.len(), 16, "both codes must march the same grid");
        assert!(worst_rod < 1e-9, "the ejection ramp must match");
        assert!(
            worst < 1e-4,
            "the C1 power history differs from the MATLAB by {worst:e}"
        );
    }

    /// **MATLAB parity on the TRANSIENT path — NEACRP D1 cold-water injection.**
    ///
    /// # Methodology
    ///
    /// The first comparison of [`crate::thdiffusion_solvertimexyz`] against the
    /// reference. D1t is the right case to start with: its **steady state
    /// already matches the MATLAB exactly**, and it has **no rod motion**, so
    /// any disagreement here is the kinetics and the transient T-H rather than
    /// the control-rod path.
    ///
    /// Both codes march the driver's generic uniform 10 ms grid over 0 to 0.5 s
    /// (51 points, 50 steps) — the case's own refined grid is dropped so the
    /// run is affordable and the two codes take identical time steps.
    ///
    /// This exercises the whole transient chain: phase-1 coupled steady state,
    /// phase-2 re-equilibration, the exponential-transform kinetics with six
    /// delayed-neutron families, analytic precursor integration, the prescribed
    /// inlet forcing, and one transient T-H step per time step.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_d1t.m`):
    ///
    /// ```text
    /// steady keff       = 0.9752852312
    /// re-equilibrated   = 0.9752774773
    /// steps marched     = 51
    /// C1 P/P0 final     = 1.0152029928   (max, at t = 0.5)
    /// C2 avg fuel T     = 787.161931 -> 787.170262
    /// C3 max fuel T     = 2476.310165 -> 2476.348375
    /// C4 coolant outlet = 553.060561 -> 552.639351
    /// precursor sum     = 1.4633302011e+01
    /// flux sum final    = 9.9268520142e+03
    ///
    /// C1 history: t=0.0 1.00000000, t=0.1 1.00187828, t=0.2 0.99914601,
    ///             t=0.3 0.99589203, t=0.4 0.99941988, t=0.5 1.01520299
    /// ```
    ///
    /// Note the power **dips** before rising — 0.9959 at t = 0.3 — so this also
    /// checks the shape of the trace, not just its endpoint.
    ///
    /// `#[ignore]`d: a coupled steady solve plus 50 transient steps.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Exact agreement across the whole transient chain.**
    ///
    /// | quantity | agreement |
    /// |---|---|
    /// | steady `k_eff` | -0.00 pcm |
    /// | re-equilibrated `k_eff` | +0.00 pcm |
    /// | steps marched | 51 = 51 |
    /// | C1 `P/P0` final | **1.6e-11** |
    /// | C2 / C3 / C4 temperatures | identical to every printed digit |
    /// | precursor sum | 2.8e-11 |
    /// | final flux sum | 2.5e-12 |
    /// | C1 history, 6 points | worst **4.3e-9** |
    ///
    /// **Interpretation.** This is the first verification of the transient
    /// path against the reference, and it covers the whole chain at once:
    /// the phase-1 coupled steady state, the phase-2 re-equilibration, the
    /// exponential-transform kinetics, analytic precursor integration over
    /// six delayed families, the prescribed inlet forcing, and the transient
    /// thermal-hydraulics.
    ///
    /// The C1 history matters as much as the endpoint: the power **dips** to
    /// 0.9959 at `t = 0.3` before rising to 1.0152, and both codes trace the
    /// same non-monotonic shape. An endpoint-only comparison could have been
    /// passed by a trajectory that got there differently.
    #[test]
    #[ignore = "MATLAB parity on the transient path; several minutes"]
    fn matlab_parity_neacrpd1t_transient() {
        const M_STEADY: f64 = 0.9752852312;
        const M_REEQ: f64 = 0.9752774773;
        const M_PREL_FINAL: f64 = 1.0152029928;
        const M_AVGFUEL_0: f64 = 787.161931;
        const M_AVGFUEL_N: f64 = 787.170262;
        const M_MAXFUEL_N: f64 = 2476.348375;
        const M_COOLOUT_0: f64 = 553.060561;
        const M_COOLOUT_N: f64 = 552.639351;
        const M_PRECURSOR: f64 = 1.4633302011e1;
        const M_FLUXSUM: f64 = 9.9268520142e3;
        // t = 0.0, 0.1, 0.2, 0.3, 0.4, 0.5
        const M_C1: [f64; 6] = [
            1.0, 1.00187828, 0.99914601, 0.99589203, 0.99941988, 1.01520299,
        ];

        let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpd1t::neacrpd1t(&Params::reference_faithful());
        // The generic uniform 10 ms grid, as the MATLAB run used.
        params.tend = Some(0.5);
        params.tgrid = None;

        let out = crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
            None,
        )
        .expect("the D1 transient should run");

        let n = out.time.len() - 1;
        let psum: f64 = (0..out.precursors_final.rows())
            .flat_map(|r| (0..out.precursors_final.cols()).map(move |c| (r, c)))
            .map(|(r, c)| out.precursors_final.get(r, c))
            .sum();
        let fsum: f64 = out.scalar_flux_final.iter().sum();

        let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
        eprintln!("D1 transient, 0 to 0.5 s — this port vs MATLAB:");
        eprintln!("  steady k_eff     {:.10}  vs {M_STEADY:.10}  ({:+.2} pcm)",
            out.steady.k_eff, (out.steady.k_eff - M_STEADY) / M_STEADY * 1e5);
        eprintln!("  re-equilibrated  {:.10}  vs {M_REEQ:.10}  ({:+.2} pcm)",
            out.k_eff, (out.k_eff - M_REEQ) / M_REEQ * 1e5);
        eprintln!("  steps marched    {}  vs 51", out.time.len());
        eprintln!("  C1 P/P0 final    {:.10}  vs {M_PREL_FINAL:.10}  rel {:.3e}",
            out.relpower[n], rel(out.relpower[n], M_PREL_FINAL));
        eprintln!("  C1 P/P0 max      {:.10} at t={:.4}  vs {M_PREL_FINAL:.10} at 0.5",
            out.prelmax, out.tpmax);
        eprintln!("  C2 avg fuel T    {:.6} -> {:.6}  vs {M_AVGFUEL_0:.6} -> {M_AVGFUEL_N:.6}",
            out.avgfueltemp[0], out.avgfueltemp[n]);
        eprintln!("  C3 max fuel T    {:.6} -> {:.6}  vs -> {M_MAXFUEL_N:.6}",
            out.maxfueltemp[0], out.maxfueltemp[n]);
        eprintln!("  C4 coolant out   {:.6} -> {:.6}  vs {M_COOLOUT_0:.6} -> {M_COOLOUT_N:.6}",
            out.coolouttemp[0], out.coolouttemp[n]);
        eprintln!("  precursor sum    {psum:.10e}  vs {M_PRECURSOR:.10e}  rel {:.3e}",
            rel(psum, M_PRECURSOR));
        eprintln!("  flux sum final   {fsum:.10e}  vs {M_FLUXSUM:.10e}  rel {:.3e}",
            rel(fsum, M_FLUXSUM));
        eprintln!("  --- C1 history ---");
        let mut worst_c1 = 0.0f64;
        for (k, want) in M_C1.iter().enumerate() {
            let i = k * 10;
            if i < out.relpower.len() {
                let got = out.relpower[i];
                let d = rel(got, *want);
                worst_c1 = worst_c1.max(d);
                eprintln!("    t={:.3}  {got:.8}  vs {want:.8}  rel {d:.3e}",
                    out.time[i]);
            }
        }
        eprintln!("  worst C1 relative difference: {worst_c1:.3e}");

        assert_eq!(out.time.len(), 51, "both codes must march the same grid");
        assert!(
            rel(out.steady.k_eff, M_STEADY) < 1e-6,
            "the steady state feeding the transient must match"
        );
        assert!(
            worst_c1 < 1e-4,
            "the C1 power history differs from the MATLAB by {worst_c1:e}"
        );
    }

    /// **MATLAB parity on NEACRP D1 — the BWR case.**
    ///
    /// # Methodology
    ///
    /// The A2 comparison verified the port on a PWR at 15.5 MPa with five
    /// feedback channels and a graded mesh. D1 is a different problem in every
    /// respect that matters: a **BWR at 6.7 MPa** whose coolant reaches
    /// saturation and boils, a **uniform** axial mesh, **two** feedback
    /// channels rather than five, 19 materials rather than 11, and a different
    /// rod geometry. It is therefore the best available second opinion.
    ///
    /// Three comparisons, matching the A2 sequence: the mesh (defect Z1 changes
    /// it), the static eigenvalue across nodal-update intervals, and the
    /// coupled solve on the `hem` path.
    ///
    /// MATLAB R2026a, 2026-08-19 (`x1_d1.m`):
    ///
    /// ```text
    /// Lz(1:3) = 30 30 30   sum(Lz(1:14)) = 420   Ztot = 426.72
    ///
    /// nodalupd            keff        residual
    ///   frozen    1.0112638927     9.7508e-07
    ///      100    1.0170868291     9.7797e-07
    ///       50    1.0172753879     8.0735e-07
    ///       20    1.0172855995     9.6828e-07
    ///        5    1.0172862168     8.2381e-07
    ///
    /// coupled, hem, nodalupd = 20:
    ///   keff = 0.9752848326
    ///   fuel T max = 1474.5021   coolant T max = 556.0312
    ///   heatflux sum = 6.684644e+04   pwrdens sum = 1.108747e+06
    /// ```
    ///
    /// **Note D1 is stable at its default interval** where A2 is not — the
    /// static eigenvalue converges cleanly at `nodalupd = 5`. So unlike A2,
    /// this case can be compared at the default as well.
    ///
    /// `#[ignore]`d: a coupled solve plus five eigensolves.
    ///
    /// # Results — measured 2026-08-19
    ///
    /// **Exact agreement on every number.**
    ///
    /// | `nodalupd` | this port | MATLAB | difference |
    /// |---|---|---|---|
    /// | frozen | 1.0112638927 | 1.0112638927 | 0.000 pcm |
    /// | 100 | 1.0170868291 | 1.0170868291 | 0.000 pcm |
    /// | 50 | 1.0172753879 | 1.0172753879 | 0.000 pcm |
    /// | 20 | 1.0172855995 | 1.0172855995 | 0.000 pcm |
    /// | 5 | 1.0172862168 | 1.0172862168 | 0.000 pcm |
    ///
    /// | coupled, `hem`, `nodalupd = 20` | MATLAB | this port |
    /// |---|---|---|
    /// | `k_eff` | 0.9752848326 | **0.9752848326** (-0.00 pcm) |
    /// | passes to converge | 27 | **27** |
    /// | fuel T max | 1474.5021 | **1474.5021** |
    /// | coolant T max | 556.0312 | **556.0312** |
    /// | heat flux sum | 6.684644e4 | **6.684644e4** |
    /// | pwrdens sum | 1.108747e6 | **1.108747e6** |
    ///
    /// **Interpretation — this is the independent second opinion.** A2 verified
    /// the port on a PWR at 15.5 MPa with a graded mesh, five feedback channels
    /// and 11 materials. D1 differs in every one of those: a **BWR at 6.7 MPa**
    /// whose coolant reaches saturation and boils, a **uniform** axial mesh,
    /// **two** feedback channels, **19** materials, a different rod geometry
    /// and a different fuel pin. Getting both exactly right is much stronger
    /// evidence than either alone, because the two cases exercise almost
    /// disjoint paths through the thermal-hydraulics.
    ///
    /// The coolant maximum of 556.0312 K is `Tsat(6.7 MPa)` — the channel
    /// boils, so this run also exercises the two-phase branch of
    /// [`crate::singleflow1devap`] that A2 never reaches.
    ///
    /// **D1 is stable at its default nodal-update interval**, converging
    /// cleanly at `nodalupd = 5`, where A2 diverges at 6. So the N1 instability
    /// is case-specific — a property of A2's heavily-rodded configuration, not
    /// a general property of small intervals.
    ///
    /// **Defect Z1 is independently confirmed here.** The mesh is 30 cm summing
    /// to 420 in both codes, against a `Ztot` of 426.72 — the same internal
    /// inconsistency as A2, on a case that writes a single scalar `30.48`
    /// rather than a graded array.
    #[test]
    #[ignore = "MATLAB parity on D1; a coupled solve, several minutes"]
    fn matlab_parity_neacrpd1_bwr() {
        // ---- 1. the mesh (defect Z1) ----
        let (params0, geometry0, ..) = crate::neacrpd1::neacrpd1(&Params::reference_faithful());
        let (_, _, maxiz) = crate::handle3dcoords::handle3dcoords(&params0);
        let zsum: f64 = geometry0.lz[..maxiz].iter().sum();
        eprintln!("D1 mesh: Lz(1:3) = {} {} {}   sum = {zsum}   (MATLAB: 30 30 30, 420)",
            geometry0.lz[0], geometry0.lz[1], geometry0.lz[2]);
        assert_eq!(geometry0.lz[0], 30.0, "Z1: the axial layer must be 30, not 30.48");
        assert!((zsum - 420.0).abs() < 1e-9);

        // ---- 2. the static eigenvalue sweep ----
        const MATLAB_STATIC: [(usize, f64); 5] = [
            (1_000_000_000, 1.0112638927),
            (100, 1.0170868291),
            (50, 1.0172753879),
            (20, 1.0172855995),
            (5, 1.0172862168),
        ];
        eprintln!("{:>12}  {:>16}  {:>16}  {:>10}", "nodalupd", "this port", "MATLAB", "pcm");
        let mut worst_static = 0.0f64;
        for (nu, want) in MATLAB_STATIC {
            let base = Params { nodalupd: nu, ..Params::reference_faithful() };
            let (params, geometry, th, whichsigma, sigmavalues, feedback) =
                crate::neacrpd1::neacrpd1(&base);
            let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
            let es = maxix * maxiy * maxiz;

            let maxir = params.fuel.maxir;
            let wk = &geometry.fuel.whichk;
            let mut sc = 0usize;
            for ir in 0..maxir - 1 {
                if (wk[ir] != 0) != (wk[ir + 1] != 0) {
                    sc += 1;
                }
            }
            let maxid = maxir + sc;
            let mut th = th;
            th.fueltempavg = vec![params.fueltempavg; es];
            th.fueltempdoppler = vec![params.fueltempavg; es];
            th.fueltemp = {
                let mut a = Array2::<f64>::zeros(es, maxid);
                for i in 0..es {
                    for j in 0..maxid {
                        a.set(i, j, params.fueltempavg);
                    }
                }
                a
            };
            th.coolant.temps = vec![params.cooltempavg; es];
            th.coolant.dens = vec![params.cooldenavg; es];
            th.heatflux = vec![0.0; es];

            let (sv, ws, _r) = sigmavalupd3d_handler(
                &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
            )
            .expect("handler");
            let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
                &geometry, &params, &sv, &ws, None, None,
            )
            .expect("eigensolve");
            let pcm = (out.k_eff - want) / want * 1e5;
            eprintln!("{nu:>12}  {:>16.10}  {want:>16.10}  {pcm:>10.3}", out.k_eff);
            worst_static = worst_static.max(pcm.abs());
        }
        eprintln!("worst static difference: {worst_static:.3} pcm");

        // ---- 3. the coupled solve ----
        const M_KEFF: f64 = 0.9752848326;
        const M_FUEL_MAX: f64 = 1474.5021;
        const M_COOL_MAX: f64 = 556.0312;
        const M_FLUX_SUM: f64 = 6.684644e4;
        const M_PWR_SUM: f64 = 1.108747e6;

        let base = Params {
            th_model: crate::types::ThModel::Hem,
            nodalupd: 20,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpd1::neacrpd1(&base);
        let out = thdiffusion_solverxyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
        )
        .expect("D1 hem should run");

        let t = &out.th;
        let ftmax = t.fueltempavg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ctmax = t.coolant.temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let qsum: f64 = t.heatflux.iter().sum();
        let psum: f64 = out.pwrdens.iter().sum();
        let pcm = (out.k_eff - M_KEFF) / M_KEFF * 1e5;

        eprintln!();
        eprintln!("D1 coupled, hem, nodalupd = 20:");
        eprintln!("  k_eff        = {:.10}  vs {M_KEFF:.10}  ({pcm:+.2} pcm)", out.k_eff);
        eprintln!("  termination  = {:?} after {} passes (MATLAB: 27)", out.termination, out.iterations);
        eprintln!("  fuel T max   = {ftmax:.4}  vs {M_FUEL_MAX:.4}");
        eprintln!("  coolant Tmax = {ctmax:.4}  vs {M_COOL_MAX:.4}");
        eprintln!("  heatflux sum = {qsum:.6e}  vs {M_FLUX_SUM:.6e}");
        eprintln!("  pwrdens sum  = {psum:.6e}  vs {M_PWR_SUM:.6e}");

        assert!(worst_static < 1.0, "static eigenvalues must agree to under 1 pcm");
        assert_eq!(out.termination, crate::thdiffusion_solverxyz::Termination::Converged);
        assert!(pcm.abs() < 100.0, "coupled k_eff is {pcm:+.2} pcm from the MATLAB");
    }

    /// **MATLAB parity on a real coupled case.** NEACRP A2, `th_model = hem`.
    ///
    /// # Methodology
    ///
    /// The MATLAB reference was **executed** on 2026-08-18 (MATLAB R2026a,
    /// `neacrpa2` through `thdiffusion_solverxyz.m`, `main_exec_diff3d.m`'s own
    /// set-up, with `params.th_model = 'hem'`). It converged:
    ///
    /// ```text
    /// T-H Iteration = 23
    /// Keff = 1.013943
    /// Fission source residual = 6.315011e-05
    /// Fuel-temp residual = 4.942020e-01 K (tol 0.5 K) [converged]
    /// ```
    ///
    /// This runs the same case through the same path in this port and compares.
    /// It is the **first direct MATLAB-parity check on a coupled solve** in the
    /// crate — everything before it compared against published values, analytic
    /// limits, or the port's own other modules.
    ///
    /// `hem` is used because it is the only functional thermal-hydraulic path
    /// in the snapshot: the default two-fluid path's 1-D kernel is missing, and
    /// on it *both* codes diverge (see the X1 write-up in
    /// `docs/bedok-reference-defects.md`).
    ///
    /// **Pass criterion: within 100 pcm of the MATLAB.** The two codes are not
    /// expected to agree bit-for-bit — different linear algebra, different
    /// iteration order — but a coupled eigenvalue agreeing to that level on a
    /// 17x17x18 five-feedback-channel case would leave no room for a
    /// translation error of consequence.
    ///
    /// `#[ignore]`d: a coupled solve, several minutes.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// # THIS TEST NOW PASSES — exactly.
    ///
    /// The history, because both steps mattered:
    ///
    /// | | MATLAB | original | after Z1 | **at `nodalupd = 20`** |
    /// |---|---|---|---|---|
    /// | termination | converged | cap at 51 | converged, 27 | **converged, 16** |
    /// | `k_eff` | 1.0139476080 | ~43.2 | 1.035684 | **1.0139476080** |
    /// | difference | - | +4.16e6 pcm | +2144 pcm | **0.0 pcm** |
    ///
    /// Reproducing **defect Z1** (the silently rounded axial mesh) turned a
    /// divergence into a converged 2144 pcm disagreement. Moving off the
    /// **unstable default nodal-update interval** (defect N1) closed the rest:
    /// at 6 the inner eigensolve diverges in both codes, so the coupled
    /// trajectories are chaotic and land on different attractors.
    ///
    /// **Resolved.** While the disagreement stood, this test was deliberately
    /// left failing rather than pinned — a pinned test would have enshrined
    /// the wrong answer — and it was `#[ignore]`d only so it did not break the
    /// suite. Both causes were then found (Z1, then N1) and it **passes
    /// exactly**, as the table above records. It stays `#[ignore]`d because a
    /// coupled A2 solve is minutes, not because anything is outstanding.
    ///
    /// **It also corrects an earlier conclusion.** The two-fluid comparison
    /// showed both codes diverging with fuel-temperature residuals agreeing to
    /// five significant figures, and I concluded from that "the translation is
    /// faithful — this is not a port bug". That conclusion was **wrong**: it
    /// only established that the two codes fail the same way on a path that is
    /// broken in both. On the path that *works*, they do not agree at all.
    ///
    /// **The most telling number is the fuel-temperature residual.** This port
    /// gives 1270.0461 K here on the `hem` path, against 1270.4035 K on the
    /// two-fluid path where the coolant is frozen by a missing solver. Those
    /// are effectively the same value — so **this port reaches the same
    /// saturated fuel state on `hem` as it does with no working coolant at
    /// all**, while the MATLAB on `hem` settles at 0.4942 K. Whatever is wrong
    /// is not subtle.
    ///
    /// Note [`crate::neacrpd1`]'s `hem` path *does* work — the coolant heats
    /// 547 K to saturation — so this is not a blanket failure of the HEM
    /// chain. It is specific to A2, which is also the case whose `crod`
    /// feedback channel the bisect showed destabilising the loop on its own.
    ///
    /// This is the crate's **first exact agreement with the MATLAB on a
    /// coupled solve** — 17x17x18, two groups, five feedback channels, fuel-rod
    /// conduction and channel thermal-hydraulics, all agreeing to ten
    /// significant figures on `k_eff` and to every printed digit on the fuel
    /// temperature, coolant temperature, heat flux and power.
    #[test]
    #[ignore = "MATLAB parity gate; a coupled solve, several minutes"]
    fn matlab_parity_a2_phase_zero_on_the_hem_path() {
        /// `Keff` the MATLAB printed for this case, measured 2026-08-18.
        const MATLAB_K_EFF: f64 = 1.0139476080;

        // `nodalupd = 20`: at the reference's default of 6 the inner
        // eigensolve is unstable on this case (defect N1) and a cold solve
        // diverges in BOTH codes, so a comparison there measures chaos rather
        // than agreement. At 20 the static solve is stable and identical.
        let base = Params {
            th_model: crate::types::ThModel::Hem,
            nodalupd: 20,
            ..Params::reference_faithful()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);
        assert_eq!(
            params.th_model,
            crate::types::ThModel::Hem,
            "the case must not override the model back to two-fluid"
        );

        let out = thdiffusion_solverxyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            Some(1.0),
        )
        .expect("A2 on the hem path should run");

        let pcm = (out.k_eff - MATLAB_K_EFF) / MATLAB_K_EFF * 1e5;
        eprintln!("NEACRP A2 Phase-0, th_model = hem:");
        eprintln!("  this port    k_eff = {:.6}", out.k_eff);
        eprintln!("  MATLAB       k_eff = {MATLAB_K_EFF:.6}");
        eprintln!("  difference         = {pcm:+.1} pcm");
        eprintln!("  termination        = {:?} after {} passes (MATLAB: 23)", out.termination, out.iterations);
        eprintln!("  fs residual        = {:.4e} (MATLAB: 6.315e-05)", out.residual);
        eprintln!("  Tfuel residual     = {:.4} K (MATLAB: 0.4942)", out.fueltemp_residual);
        eprintln!("  Tfuel converged    = {}", out.fueltemp_converged);

        assert_eq!(
            out.termination,
            crate::thdiffusion_solverxyz::Termination::Converged,
            "the MATLAB converges on this case, so this port must too"
        );
        assert!(
            pcm.abs() < 100.0,
            "k_eff = {:.6} is {pcm:+.1} pcm from the MATLAB's {MATLAB_K_EFF:.6}",
            out.k_eff
        );
    }

    /// **X1 bisect: which feedback channel destabilises pass 16?**
    ///
    /// # Methodology
    ///
    /// The Phase-0 solve on [`crate::neacrpa2`] converges to a `k_eff = 1.0355`
    /// plateau over passes 7-15 and then diverges. A1, whose feedback is
    /// effectively switched off by its 1e-6 power ratio, converges cleanly. So
    /// the suspicion is one of the five feedback channels.
    ///
    /// This runs the Phase-0 solve with the channels enabled **one at a time**,
    /// plus none and all, and reports for each the pass at which `k_eff` first
    /// leaves a plausible band. Capped at 25 outer passes: the event is at 16,
    /// so 25 sees it without paying for all 51.
    ///
    /// A channel that diverges alone is implicated. If none does alone but all
    /// together do, the instability is in their *interaction* — or in the
    /// driver rather than the physics.
    ///
    /// `#[ignore]`d: seven coupled solves.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | channels | diverged at | `k_eff` | passes |
    /// |---|---|---|---|
    /// | none | - | 1.0012 | 10 |
    /// | boron only | - | 1.0214 | 10 |
    /// | fueltemp only | - | 1.0014 | 10 |
    /// | cooltemp only | - | 1.0018 | 10 |
    /// | coolden only | - | 1.0094 | 10 |
    /// | **crod only** | **7** | **394.72** | 26 |
    /// | ALL five | 15 | 15.18 | 26 |
    ///
    /// **The control-rod channel is the destabiliser.** Every other channel
    /// converges alone in 10 passes, and so does no-feedback-at-all. `crod`
    /// alone diverges immediately — `1.000, 65.77, 314.48, 462.67, ...` — and
    /// never recovers.
    ///
    /// **This also corrects the diagnosis from the MATLAB cross-check.** That
    /// run showed the default two-fluid path leaving the coolant frozen, and
    /// the reasonable inference was that no fixed point exists when power rises
    /// and the coolant cannot respond. That inference is **too strong**: the
    /// `none` row here has the same frozen coolant and converges perfectly
    /// well. A frozen coolant is not by itself fatal; the rod feedback on top
    /// of it is.
    ///
    /// **Why this is consistent with the MATLAB converging on `hem`.** There,
    /// all five channels are live *and* the coolant works, and it reaches
    /// `k_eff = 1.013943` in 23 iterations. So the rod channel is not broken in
    /// isolation — it is **sensitive to the coolant state**, and only
    /// destabilises the loop when the coolant is frozen.
    ///
    /// **A candidate mechanism, not yet tested.** The rod channel is the one
    /// that most enlarges the material table — feedback splits 11 base
    /// materials into 3978 rows on this case — and
    /// [`crate::calc_bucklingxyz`]'s cache is fingerprinted on three sums and
    /// three non-zero counts, which cannot separate every distinct
    /// cross-section set. A collision silently reuses the wrong cached
    /// coefficients, and more rows means more chances to collide. That defect
    /// is already registered; this would be the first evidence of it firing.
    ///
    /// **Also worth noting:** the rod slope's `reference` is forced to zero on
    /// use, so a fully rodded node takes the *entire* slope rather than a
    /// departure from a reference state — a much larger perturbation than the
    /// other four channels apply.
    ///
    /// Neither has been confirmed. The next step is to compare against the
    /// MATLAB on the same configuration, since a faithful translation should
    /// destabilise there too.
    #[test]
    #[ignore = "X1 bisect; seven coupled solves, ~20 min"]
    fn x1_bisect_which_feedback_channel_destabilises_the_loop() {
        let (base_params, geometry, th, whichsigma, sigmavalues, full) =
            crate::neacrpa2::neacrpa2(&Params::reference_faithful());

        let only = |pick: &str| -> FeedbackTables {
            let mut f = FeedbackTables::default();
            match pick {
                "boron" => f.boron = full.boron.clone(),
                "fueltemp" => f.fueltemp = full.fueltemp.clone(),
                "cooltemp" => f.cooltemp = full.cooltemp.clone(),
                "coolden" => f.coolden = full.coolden.clone(),
                "crod" => f.crod = full.crod.clone(),
                _ => {}
            }
            f
        };

        let cases: Vec<(&str, FeedbackTables)> = vec![
            ("none", FeedbackTables::default()),
            ("boron only", only("boron")),
            ("fueltemp only", only("fueltemp")),
            ("cooltemp only", only("cooltemp")),
            ("coolden only", only("coolden")),
            ("crod only", only("crod")),
            ("ALL five", full.clone()),
        ];

        eprintln!("{:<16} {:>10} {:>12} {:>10}  first 18 k_eff", "channels", "diverged", "k_eff", "passes");
        for (name, feedback) in cases {
            let params = Params {
                thmaxiter: Some(25),
                ..base_params.clone()
            };
            let r = thdiffusion_solverxyz(
                &geometry,
                &params,
                &th,
                &sigmavalues,
                &feedback,
                &whichsigma,
                Some(1.0),
            );
            match r {
                Ok(o) => {
                    // The first pass after the initial cold-start transient at
                    // which k_eff leaves a plausible band and does not return.
                    let diverged_at = o
                        .k_eff_history
                        .iter()
                        .enumerate()
                        .skip(7)
                        .find(|(_, k)| !k.is_finite() || (**k - 1.0).abs() > 0.5)
                        .map(|(i, _)| i.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let hist: Vec<String> = o
                        .k_eff_history
                        .iter()
                        .take(18)
                        .map(|k| {
                            if k.is_finite() && k.abs() < 1e4 {
                                format!("{k:.3}")
                            } else {
                                format!("{k:.1e}")
                            }
                        })
                        .collect();
                    eprintln!(
                        "{:<16} {:>10} {:>12.4} {:>10}  [{}]",
                        name,
                        diverged_at,
                        o.k_eff,
                        o.iterations,
                        hist.join(", ")
                    );
                }
                Err(e) => eprintln!("{name:<16} ERROR: {e}"),
            }
        }
        eprintln!();
        eprintln!("'diverged' = first pass >= 7 where |k_eff - 1| > 0.5; '-' means it never did.");
    }

    /// **Does the G1 correction fix X1?** The payoff experiment.
    ///
    /// # Methodology
    ///
    /// Two runs, both with
    /// [`crate::types::GradDForm`] enabled:
    ///
    /// 1. **IAEA-3D** — a *uniform* mesh, so the correction is provably a
    ///    no-op. `k_eff` must come back **exactly** 1.029084. This is the
    ///    control: if it moves, the correction has a side effect and nothing
    ///    else here can be believed.
    /// 2. **NEACRP A2's Phase-0 coupled solve** — a graded mesh, where the
    ///    correction bites. The X1 diagnostic showed this loop converging to
    ///    `k_eff ~ 1.0355` over passes 7-15 and then destabilising at pass 16.
    ///    If a merely *inaccurate* operator were the problem, correcting it
    ///    would shift the answer; if an *inconsistent* one was destabilising
    ///    the feedback iteration, correcting it should let the loop converge.
    ///
    /// `#[ignore]`d: a coupled solve, several minutes.
    ///
    /// # Results — measured 2026-08-22 (REVERSING a 2026-08-18 conclusion)
    ///
    /// **Control passed.** IAEA-3D with the correction on converges to
    /// `k_eff = 1.029084` — unchanged, as the uniform mesh requires.
    ///
    /// **A2's Phase-0 solve, at the case's default nodal interval:**
    ///
    /// | operator | termination | `k_eff` | usable by Phase 0? |
    /// |---|---|---|---|
    /// | reference (G1/G2/G3 present) | **IterationCap at 51** | 184.52 | no |
    /// | conservative (corrected) | **Converged in 10** | 1.026437 | **yes** |
    ///
    /// The corrected arm's trace settles immediately —
    /// `1.0000, 1.0251, 1.0262, 1.0262, ...` — with a fission-source residual
    /// of 2.4e-5 and a fuel-temperature residual of 0.32 K. The reference arm
    /// wanders over four orders of magnitude and never recovers:
    /// `1.0000, 87.24, 2069.48, 136.30, 381.64, ...`.
    ///
    /// # This reverses what this test concluded on 2026-08-18
    ///
    /// It then recorded: *"The hypothesis is REFUTED. A2's Phase-0 solve does
    /// not converge with the correction, and is worse than without it"* —
    /// neither arm converged, and the corrected one looked the wilder of the
    /// two (never plateauing, against a 1.0355 plateau over passes 7-15). That
    /// finding was carried into the defect register as evidence **against**
    /// defaulting the correction on.
    ///
    /// **It was wrong, and the reason is defect G3.** At the time the
    /// correction was applied to the diffusion operator but not to
    /// `gradterms`, leaving the two more inconsistent than the reference had
    /// them — so the SA-nodal cancellation stopped working and the power
    /// distribution was corrupted. What was being measured was a half-applied
    /// correction, not the correction. With G1, G2 and G3 corrected together
    /// the conclusion inverts: the corrected operator is the one that
    /// converges.
    ///
    /// **What the original hypothesis asked is still answered "no", though.**
    /// G1 was never the cause of X1 — X1's causes were defect **Z1** and
    /// defect **N1**, both since found. The correction improves this case's
    /// Phase-0 convergence markedly, but that is a separate and later finding,
    /// not a vindication of the original guess.
    ///
    /// The lesson worth keeping: **a negative result from a partially applied
    /// change is not a result about the change.**
    #[test]
    #[ignore = "G1/X1 experiment; a coupled solve, several minutes"]
    fn does_the_g1_correction_fix_x1() {
        use crate::types::GradDForm;

        // --- control: uniform mesh, must not move ---
        let corrected = Params {
            nodalupd: 6,
            gradd_form: GradDForm::Conservative,
            ..Default::default()
        };
        let (p, g, w, sv) = crate::iaea3ds::iaea3ds(&corrected);
        let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
            &g, &p, &sv, &w, None, None,
        )
        .unwrap();
        eprintln!("IAEA-3D (uniform mesh) with the correction ON:");
        eprintln!("  k_eff = {:.6} (must be 1.029084)", out.k_eff);
        eprintln!("  termination = {:?}", out.termination);
        assert!(
            (out.k_eff - 1.029_084).abs() < 5e-7,
            "the correction must not move a uniform-mesh case; got {}",
            out.k_eff
        );

        // --- the experiment: A2's Phase-0 solve, graded mesh ---
        for conservative in [false, true] {
            let base = Params {
                gradd_form: if conservative { GradDForm::Conservative } else { GradDForm::Reference },
                ..Default::default()
            };
            let (params, geometry, th, whichsigma, sigmavalues, feedback) =
                crate::neacrpa2::neacrpa2(&base);

            let r = thdiffusion_solverxyz(
                &geometry,
                &params,
                &th,
                &sigmavalues,
                &feedback,
                &whichsigma,
                Some(1.0),
            );
            eprintln!(
                "NEACRP A2 Phase-0, gradd_conservative = {conservative}:"
            );
            match r {
                Ok(o) => {
                    eprintln!("  termination = {:?} after {} passes", o.termination, o.iterations);
                    eprintln!("  k_eff       = {:.6}", o.k_eff);
                    eprintln!("  fs residual = {:.4e}", o.residual);
                    eprintln!("  Tfuel resid = {:.4} K", o.fueltemp_residual);
                    let usable = o.k_eff.is_finite() && o.k_eff > 0.8 && o.k_eff < 1.2;
                    eprintln!("  usable by Phase 0? {usable}");
                    let hist: Vec<String> = o
                        .k_eff_history
                        .iter()
                        .take(20)
                        .map(|k| format!("{k:.4}"))
                        .collect();
                    eprintln!("  k_eff history (first 20) = [{}]", hist.join(", "));
                }
                Err(e) => eprintln!("  ERROR: {e}"),
            }
        }
    }

    /// **X1 diagnostic: what does the Phase-0 coupled solve actually do on A2?**
    ///
    /// # Methodology
    ///
    /// X1 was narrowed to "A2 fell back to the bootstrap and A1 did not". That
    /// leaves two sub-questions, and this answers the first: *why* did the
    /// standard coupled solver fail on A2? It calls
    /// [`crate::thdiffusion_solverxyz`] directly on
    /// [`crate::neacrpa2`] — exactly what Phase 0 does — and prints the
    /// eigenvalue history, so the failure mode is visible rather than inferred
    /// from a single out-of-range `k_eff`.
    ///
    /// Run against A1 too, which is known to succeed, so the two are directly
    /// comparable.
    ///
    /// `#[ignore]`d: two coupled solves, several minutes.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **A1 converges in 7 passes** to `k_eff = 0.999972`, fuel-temperature
    /// residual 0, monotone throughout: `1.0000, 0.9980, 0.9994, 0.9996,
    /// 0.9998, 0.9999, 0.9999, 1.0000`.
    ///
    /// **A2 hits the iteration cap at pass 51 with `k_eff = 45.78`** — but the
    /// shape of the failure is the finding, not the number:
    ///
    /// ```text
    /// pass  1-6   1.0000, 21.60, 37512.74, 64.42, 5032.75, 376.04
    /// pass  7-15  1.0347, 1.0351, 1.0353, 1.0354, 1.0355, 1.0355, 1.0355, 1.0355, 1.0355
    /// pass 16+    1556.64, 812.64, 41.93, 595.10, ... (never recovers)
    /// ```
    ///
    /// with the fuel-temperature residual over passes 7-12 reading
    /// `6.405e2, 3.203e2, 1.601e2, 8.006e1, 4.003e1, 2.002e1`.
    ///
    /// **Interpretation — the loop was converging and then lost it.** Passes
    /// 7 to 15 are a converging fixed-point iteration: `k_eff` is flat to five
    /// digits at 1.0355 and the temperature residual halves exactly each pass,
    /// which is the `wrelax = 0.5` under-relaxation approaching a *stationary*
    /// target. Then pass 16 diverges by three orders of magnitude.
    ///
    /// So this is **not** a bad initial guess that never recovers, and not the
    /// cold start alone — the cold start is passes 1-6, and it *did* recover.
    /// Something destabilises an already-converging iteration.
    ///
    /// **What differs between A1 and A2 here.** A1 is hot zero power: at a
    /// `powratio` of 1e-6 the feedback is effectively switched off, which is
    /// why it converges monotonically. A2 is full power with all five feedback
    /// channels live. And A2 is a **graded axial mesh**, where
    /// [`crate::makegrad_dxyz`]'s operator carries defect G1 — misstating the
    /// face coupling by up to +144.8% at the bottom of the core.
    ///
    /// That yields a testable hypothesis: **G1 may be the cause of X1.** A
    /// corrupted operator that is merely inaccurate would bias `k_eff`; one
    /// that is inconsistent can also destabilise a feedback loop that keeps
    /// re-solving against it. [`crate::neacrpd1`], which converges in 12
    /// passes, has a **uniform** axial mesh — consistent with the hypothesis.
    #[test]
    #[ignore = "X1 diagnostic; two coupled solves, several minutes"]
    fn x1_what_the_phase_zero_solve_does_on_a2_versus_a1() {
        for (name, built) in [
            ("A2 (full power)", crate::neacrpa2::neacrpa2(&Params::reference_faithful())),
            ("A1 (HZP)", crate::neacrpa1t::neacrpa1t(&Params::reference_faithful())),
        ] {
            let (params, geometry, th, whichsigma, sigmavalues, feedback) = built;
            eprintln!("===== {name} =====");
            eprintln!("  starting boron = {:.2} ppm", params.boron);

            let out = thdiffusion_solverxyz(
                &geometry,
                &params,
                &th,
                &sigmavalues,
                &feedback,
                &whichsigma,
                Some(1.0),
            );

            match out {
                Ok(o) => {
                    eprintln!("  termination    = {:?} after {} passes", o.termination, o.iterations);
                    eprintln!("  k_eff          = {:.6}", o.k_eff);
                    eprintln!("  fs residual    = {:.4e}", o.residual);
                    eprintln!("  Tfuel residual = {:.4} K", o.fueltemp_residual);
                    let usable = o.k_eff.is_finite() && o.k_eff > 0.8 && o.k_eff < 1.2;
                    eprintln!("  usable by Phase 0 (0.8 < k < 1.2)? {usable}");
                    let hist: Vec<String> = o
                        .k_eff_history
                        .iter()
                        .map(|k| {
                            if k.is_finite() {
                                format!("{k:.4}")
                            } else {
                                format!("{k}")
                            }
                        })
                        .collect();
                    eprintln!("  k_eff history  = [{}]", hist.join(", "));
                    let ft: Vec<String> = o
                        .fueltemp_residual_history
                        .iter()
                        .take(12)
                        .map(|r| format!("{r:.3e}"))
                        .collect();
                    eprintln!("  Tfuel history  = [{}] (first 12)", ft.join(", "));
                }
                Err(e) => eprintln!("  ERROR: {e}"),
            }
        }
    }

    /// **Diagnostic for X1: the same search on case A1.**
    ///
    /// # Methodology
    ///
    /// [`crate::neacrpa1t`] is a different configuration of the same core —
    /// hot zero power, six of seven banks fully inserted, a much lower critical
    /// boron. Running the identical search on it gives a **second, independent
    /// data point** on the X1 discrepancy recorded above.
    ///
    /// The logic: if this port is systematically more reactive than the MATLAB,
    /// A1 should overshoot its 551.31 ppm reference by a *similar relative*
    /// amount to A2's +10%. If instead A1 lands close, the A2 gap is specific to
    /// that configuration — most likely the bootstrap path — and not a general
    /// translation error.
    ///
    /// `#[ignore]`d because it costs about ten minutes; it is a diagnostic to
    /// run deliberately, not a gate. Run with
    /// `cargo test -p bedok --release -- --ignored the_search_on_case_a1`.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | | A1 (this test) | A2 |
    /// |---|---|---|
    /// | critical boron | **551.14 ppm** | 1253.29 ppm |
    /// | the reference's own | 551.31 ppm | 1139.01 ppm |
    /// | difference | **-0.17 ppm (-0.03%)** | **+114.28 ppm (+10.03%)** |
    /// | boron worth slope | -9.75 pcm/ppm | -9.62 pcm/ppm |
    /// | Phase 0 bootstrapped | **no** | **yes** |
    ///
    /// `k_eff` = 1.000006, converged, in 149 s.
    ///
    /// **This localises X1, and largely exonerates the translation.**
    ///
    /// On A1 this port reproduces the reference's own critical boron to
    /// **0.03%** — 0.17 ppm out of 551, which at -9.75 pcm/ppm is under
    /// **2 pcm**. A1 and A2 share the cross sections, the material map, the
    /// feedback chain including the boron channel, the eigensolver, the
    /// thermal-hydraulics and this entire search. If any of those were
    /// mistranslated, A1 could not land within 2 pcm of the reference.
    ///
    /// The one thing that differs between the two runs is **which Phase-0 path
    /// was taken**: A1 got a usable coupled state from the standard solver, A2
    /// did not and fell back to the bootstrap. That is now the prime suspect,
    /// and it splits into two sub-questions:
    ///
    /// 1. **Does the bootstrap converge a wrong state?** It freezes the nodal
    ///    correction and under-relaxes at fixed boron, so it may settle
    ///    somewhere the standard solver would not.
    /// 2. **Why did [`crate::thdiffusion_solverxyz`] fail on A2 at all?** The
    ///    reference presumably did not need a fallback to reach 1139.01. If the
    ///    coupled driver fails here where the MATLAB succeeds, that is the real
    ///    defect and the bootstrap is only exposing it.
    ///
    /// Also worth recording: A1's result sits **-16.56 ppm** from the published
    /// 567.7 ppm, almost exactly the **-16.39 ppm** the reference itself
    /// reports. So on the case where the bootstrap is not involved, this port
    /// reproduces not only the reference's answer but its *disagreement with the
    /// benchmark* — which is what a faithful translation should do.
    #[test]
    #[ignore = "expensive X1 diagnostic (~10 min); run deliberately"]
    fn the_search_on_case_a1_gives_a_second_data_point_for_x1() {
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa1t::neacrpa1t(&Params::reference_faithful());

        let out = criticalboron_xyz(
            &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, None, None,
        )
        .expect("the boron search should run on case A1");

        let reference = crate::neacrpa1t::CRITICAL_BORON;
        let bench = crate::neacrpa1t::BENCHMARK_CRITICAL_BORON;
        let d_ref = out.boron - reference;
        eprintln!("NEACRP A1 (HZP) critical-boron search:");
        eprintln!("  started at        = {:.2} ppm", params.boron);
        eprintln!("  critical boron    = {:.2} ppm", out.boron);
        eprintln!("  k_eff             = {:.6}", out.k_eff);
        eprintln!("  boron worth slope = {:.2} pcm/ppm", out.slope_pcm_per_ppm);
        eprintln!("  converged         = {}", out.converged);
        eprintln!("  bootstrapped      = {}", out.bootstrapped);
        eprintln!("  vs reference      = {reference:.2} ppm -> {d_ref:+.2} ppm ({:+.2}%)",
            d_ref / reference * 100.0);
        eprintln!("  vs benchmark      = {bench:.1} ppm -> {:+.2} ppm", out.boron - bench);
        eprintln!(
            "  X1 comparison: case A2 was +114.28 ppm (+10.03%) over its 1139.01 reference"
        );

        assert!(out.slope_pcm_per_ppm < 0.0, "boron worth must be negative");
        assert!(
            out.boron > 0.0 && out.boron < 2000.0,
            "critical boron {} ppm is implausible for HZP",
            out.boron
        );
    }

    /// **The search runs on a real case and finds a critical state.**
    ///
    /// # Methodology
    ///
    /// [`crate::neacrpa2`] at its 1000 ppm starting point, which is
    /// sub-critical of the case's own critical value, so the search has real
    /// work to do. The full structure runs: a Phase-0 coupled solve, the
    /// frozen-T-H secant, and the warm coupled refinement.
    ///
    /// Pass criteria, all structural: the search returns; the boron worth slope
    /// is **negative** (boron is an absorber, so more of it must lower
    /// `k_eff`); the final `k_eff` is much closer to 1 than the starting one;
    /// and the concentration lands in a physically plausible band for a PWR.
    ///
    /// **This is not a comparison against the published 1160.6 ppm.** The
    /// settings that produced the reference's own 1139.01 ppm are unknown —
    /// `test_critboron3.m` is not in the snapshot — so the two are not
    /// like-for-like, and the graded-mesh defect G1 sits underneath both. What
    /// the run does is report where this port lands, so the number exists.
    ///
    /// # Results — measured 2026-08-22 (superseding a 2026-08-18 run)
    ///
    /// **Converged**, in 4 secant + 5 coupled iterations, **without the
    /// bootstrap**.
    ///
    /// | | |
    /// |---|---|
    /// | started at | 1000.00 ppm (`k_eff` = 1.0153) |
    /// | **critical boron** | **1153.13 ppm** |
    /// | `k_eff` there | 0.999999 |
    /// | boron worth slope | **-9.87 pcm/ppm** |
    /// | Phase 0 needed the bootstrap | **no** |
    /// | this code's own quoted value | 1139.01 ppm |
    /// | published benchmark (PANTHER) | 1160.6 ppm |
    ///
    /// **Interpretation.** Two things changed since the 2026-08-18 run, and
    /// both are improvements rather than drift.
    ///
    /// *The search no longer needs the bootstrap.* It previously reported
    /// `bootstrapped == true` because Phase 0's standard coupled solve did not
    /// produce a usable state on this case; it now converges directly. That
    /// removes the concern recorded here at the time that Phases 1 and 2 were
    /// searching around a state the standard solver would not have reached.
    ///
    /// *The answer moved from 1253.29 to 1153.13 ppm*, i.e. from **+114 ppm
    /// above** this code's own 1139.01 to **+14 ppm above** it, and from
    /// +92.7 ppm above the published 1160.6 to **-7.5 ppm below** it. The
    /// -1100 pcm "open discrepancy" recorded here on 2026-08-18 was **X1**,
    /// and it is resolved: its causes were defect **Z1** (the silently rounded
    /// axial mesh) and defect **N1** (the unstable default nodal-update
    /// interval). None of the three candidates listed at the time — an
    /// unlike-for-like comparison, the bootstrap state, or a translation error
    /// in the feedback chain — was the cause.
    ///
    /// **An independent corroboration falls out of this.** The G1/G2/G3
    /// correction work estimated A2's corrected critical boron at **~1152.5
    /// ppm** from a two-point secant through coupled solves at 1000 and 1100
    /// ppm (see `crate::makegrad_dxyz`). This full search, which uses a
    /// different algorithm and a different starting point, lands at **1153.13
    /// ppm** — 0.6 ppm apart. Two independent routes agreeing to that
    /// tolerance is worth more than either number alone.
    ///
    /// The remaining **-7.5 ppm against PANTHER** is unexplained and is not
    /// attributed here.
    ///
    /// The test still asserts only what is defensible — a negative slope,
    /// movement toward criticality, and a plausible PWR band — and
    /// deliberately does **not** pin agreement with 1139.01 or 1160.6.
    #[test]
    fn the_search_finds_a_critical_boron_on_the_pwr_case() {
        // `th_model = Hem`: the default two-fluid path cannot run at all in
        // this snapshot — its 1-D kernel is missing, and the MATLAB fails on it
        // identically (21 `Undefined function` warnings, no convergence). A
        // search over a non-functional T-H path is not a meaningful test.
        let base = Params {
            th_model: crate::types::ThModel::Hem,
            ..Default::default()
        };
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&base);

        let out = criticalboron_xyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
            None,
        )
        .expect("the boron search should run on case A2");

        eprintln!("NEACRP A2 critical-boron search:");
        eprintln!("  started at        = {:.2} ppm", params.boron);
        eprintln!("  critical boron    = {:.2} ppm", out.boron);
        eprintln!("  k_eff             = {:.6}", out.k_eff);
        eprintln!("  boron worth slope = {:.2} pcm/ppm", out.slope_pcm_per_ppm);
        eprintln!(
            "  iterations        = {} secant + {} coupled",
            out.secant_iterations, out.coupled_iterations
        );
        eprintln!("  converged         = {}", out.converged);
        eprintln!("  bootstrapped      = {}", out.bootstrapped);
        eprintln!("  k_eff history     = {:?}",
            out.k_eff_history.iter().map(|k| (k * 1e4).round() / 1e4).collect::<Vec<_>>());
        eprintln!(
            "  reference's own value = {:.2} ppm; published benchmark = {:.1} ppm",
            crate::neacrpa2t::CRITICAL_BORON,
            crate::neacrpa2t::BENCHMARK_CRITICAL_BORON
        );

        // Boron is an absorber.
        assert!(
            out.slope_pcm_per_ppm < 0.0,
            "boron worth must be negative, got {}",
            out.slope_pcm_per_ppm
        );
        // The search moved toward criticality.
        let started = out.k_eff_history[0];
        eprintln!("  |k-1|: {:.2e} -> {:.2e}", (started - 1.0).abs(), (out.k_eff - 1.0).abs());
        assert!(
            (out.k_eff - 1.0).abs() < (started - 1.0).abs(),
            "the search should end closer to critical than it started"
        );
        // A physically plausible PWR band.
        assert!(
            out.boron > 0.0 && out.boron < 3000.0,
            "critical boron {} ppm is outside any plausible PWR range",
            out.boron
        );
        assert!(out.boron_history.len() == out.k_eff_history.len());
    }
}
