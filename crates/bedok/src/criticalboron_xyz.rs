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
) -> Result<(f64, Vec<f64>, Vec<f64>)> {
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
        if res < 1e-8 && kres < 1e-9 {
            break;
        }
    }
    Ok((k, phi, fs))
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
                    let (k, p, fsb) = eigsolve_cold(
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
            crate::neacrpa1t::neacrpa1t(&Params::default());

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
    /// # Results — measured 2026-08-18
    ///
    /// **Converged**, in 4 secant + 6 coupled iterations.
    ///
    /// | | |
    /// |---|---|
    /// | started at | 1000.00 ppm (`k_eff` = 1.0249) |
    /// | **critical boron** | **1253.29 ppm** |
    /// | `k_eff` there | 1.000001 |
    /// | boron worth slope | **-9.62 pcm/ppm** |
    /// | Phase 0 needed the bootstrap | **yes** |
    ///
    /// **Two findings, and the second is a problem.**
    ///
    /// *First, the machinery works.* The search drove `|k_eff - 1|` from
    /// 2.49e-2 to 7.87e-7 and met both convergence criteria. The measured
    /// boron worth of -9.62 pcm/ppm is close to the reference's own
    /// -9 pcm/ppm seed and is a normal PWR value, so the slope the secant
    /// measures is physically sensible.
    ///
    /// Note `bootstrapped == true`: the standard coupled solver **did not**
    /// produce a usable Phase-0 state on this case and the fallback loop was
    /// needed. That is exactly the cold-start failure the reference's
    /// comments describe for this heavily-rodded configuration, so the
    /// bootstrap path is not dead code — it is the path case A2 takes.
    ///
    /// *Second, the answer disagrees with the reference's own.*
    ///
    /// | source | ppm |
    /// |---|---|
    /// | **this port** | **1253.29** |
    /// | the reference MATLAB | 1139.01 |
    /// | published benchmark (PANTHER) | 1160.6 |
    ///
    /// That is **+114 ppm above the reference**, which at the measured
    /// -9.62 pcm/ppm is roughly **1100 pcm** of reactivity. It is far too
    /// large to be round-off or tolerance choice: this port computes a
    /// materially more reactive core than the MATLAB does, and needs more
    /// boron to hold it critical.
    ///
    /// **This is an open discrepancy and it is not attributed.** Candidates,
    /// none eliminated:
    ///
    /// - **The comparison may not be like-for-like.** The 1139.01 figure
    ///   came from `test_critboron3.m`, which is not in the snapshot, so its
    ///   starting point, tolerances and T-H model are unknown. It may not
    ///   have taken the bootstrap path this run did.
    /// - **The bootstrap state.** Phase 0 fell back here, and the bootstrap
    ///   converges a *different* coupled state than the standard solver
    ///   would; Phases 1 and 2 then search around it.
    /// - **A translation error somewhere in the feedback chain.** Case A2 is
    ///   the first to drive all five channels, and boron is one of the three
    ///   this crate had never exercised before `neacrpa2` landed.
    ///
    /// Defect G1 (the graded-mesh face coupling) is **not** a candidate on
    /// its own: the reference carries it identically, so it cannot explain a
    /// difference *between* the two.
    ///
    /// The test asserts only what is defensible — a negative slope, movement
    /// toward criticality, and a plausible PWR band. **It deliberately does
    /// not assert agreement with 1139.01 or 1160.6**, because that agreement
    /// does not exist and pinning a wrong number would hide it.
    #[test]
    fn the_search_finds_a_critical_boron_on_the_pwr_case() {
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            crate::neacrpa2::neacrpa2(&Params::default());

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
