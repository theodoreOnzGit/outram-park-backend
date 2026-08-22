//! Transient coupled neutronics / thermal-hydraulics — the time-dependent
//! counterpart of [`crate::thdiffusion_solverxyz`].
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `thdiffusion_solvertimexyz.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What it does, in three phases
//!
//! Written for the NEACRP-L-335 rod-ejection and cold-water-injection
//! transients.
//!
//! 1. **Initial steady state.** [`crate::thdiffusion_solverxyz`] is run to
//!    convergence, and the transient fission operator is then divided by the
//!    resulting `k_eff` so the transient starts exactly critical. That stands
//!    in for the critical-boron search the benchmark performs to the same end.
//! 2. **Rebuild and re-equilibrate.** The diffusion operator is reassembled at
//!    the steady state and the flux and eigenvalue are re-equilibrated on it
//!    with a power iteration, so time stepping starts from an exact
//!    equilibrium of the operator it will actually use — not of a slightly
//!    different one.
//! 3. **Time integration** of the two-group diffusion equation with six
//!    delayed-neutron precursor families, the prescribed control-assembly
//!    motion, and one transient T-H step per time step.
//!
//! # The two kinetics schemes
//!
//! [`TimeScheme::ExponentialTransform`] is the default and the interesting
//! one: an exponential-transform implicit Euler for the flux with **analytic**
//! precursor integration, assuming the transformed fission source varies
//! linearly over the step. It is the scheme of the nodal program Ants
//! (A. Rintala, U. Lauranto, *Ann. Nucl. Energy* **190** (2023) 109868,
//! Eqs. (3)-(13)).
//!
//! The frequencies are iterated **within** the step — a predictor pass at
//! `omega = 0`, then `freqiter - 1` correctors recomputed from the newest flux
//! of the current step. The reference records that extrapolating them from the
//! previous step instead proved unstable against the lagged T-H feedback,
//! producing a growing two-step power oscillation, so it is not done.
//!
//! [`TimeScheme::ImplicitEuler`] is the reference's own "legacy" first-order
//! scheme: plain implicit Euler for both flux and precursors, with the
//! precursors eliminated analytically into the flux equation.
//!
//! # The `omega*dt` clamp is physics, not overflow protection
//!
//! The per-step exponent is clamped to `[-0.9, 2]`. The reference is explicit
//! that this is a **physical** bound: the upper limit keeps the transform
//! effective for the global mode (7.4x growth per step) while bounding
//! pathological extrapolation, and the lower limit keeps the transformed
//! time-derivative coefficient `omega + 1/dt` positive. Reproduced exactly.
//!
//! # Three deliberate departures from the reference
//!
//! Each follows a precedent already set elsewhere in this crate, and none
//! changes a number.
//!
//! 1. **The `.mat` steady-state cache is not translated.** The reference's
//!    `params.steadyfile` loads or saves a MATLAB `.mat` file around phase 1.
//!    That format is MATLAB's, and a library that silently reads a cache keyed
//!    only on a filename — which the reference's own comment warns must be
//!    deleted after any change to the case or params — is a correctness trap.
//!    The Rust signature takes `initial_steady: Option<&CoupledOutput>`
//!    instead, so a caller that wants the caching does it explicitly and owns
//!    the invalidation.
//! 2. **The CSV and JPG writes are returned, not written.** Six `writetable` /
//!    `writematrix` calls and a `saveas` become fields on
//!    [`TransientOutput`], as the flux solvers' diagnostics already do.
//! 3. **`th.inlettemp_t` is an enum, not a function handle.** See
//!    [`crate::types::InletForcing`].
//!
//! # A reference quirk in the C5/C6 radial maps
//!
//! The output maps are taken at "active-core axial layers 6 and 13", with
//! layer `L` spanning mesh layers `L*zscale + 1 ..= (L+1)*zscale`. That
//! indexing assumes the **PWR** model's 18 axial blocks (1 lower reflector,
//! 2-17 active, 18 upper reflector). Case D1 has only 14 layers, so its
//! "layer 13" lands on the top reflector rather than inside the core.
//! Reproduced as written, per the no-silent-repairs policy, and pinned by a
//! test.
//!
//! # Verification status
//!
//! **The driver marches.** On [`crate::neacrpd1t`] — NEACRP case D1 cold-water
//! injection — it completes without tripping the divergence guard, starts at
//! exactly `P/P0 = 1`, and moves the power the right way: colder inlet water
//! means a denser moderator, more reactivity, and a rising power (+1.34% over
//! 0.5 s) while the coolant outlet falls. Precursor concentrations stay
//! non-negative throughout. Measured 2026-08-18.
//!
//! **The two kinetics schemes agree to 3.2e-6** on the same window. That is the
//! strongest evidence here: the exponential-transform and implicit-Euler paths
//! share the operator assembly but implement the kinetics algebra completely
//! separately, so their agreement tests the part of this module that is unique
//! to it.
//!
//! **Nothing here has been compared to a published transient result.** The
//! NEACRP specification is not in `crates/kovan-literature`, so there is no C1
//! power curve to judge against, and the tests assert structure and
//! cross-scheme consistency only. Do not describe the transient path as
//! validated.

use crate::error::Result;
use crate::matlab::{norm1, norm2, Array2, Array3, Decomposition, SparseMatrix};
use crate::sigmavalupd3d_handler::{sigmavalupd3d_handler, FeedbackTables};
use crate::thdiffusion_solverxyz::{thdiffusion_solverxyz, CoupledOutput};
use crate::types::{FreqMode, Geometry, Params, SigmaValues, Th, TimeScheme};

/// The reference's transient defaults.
pub mod defaults {
    /// `timepicard` — T-H feedback Picard passes per step.
    pub const PICARD: usize = 1;
    /// `nodalupdtime` — SA-nodal update interval, in steps.
    pub const NODAL_UPDATE: usize = 1;
    /// `freqiter` — flux solves per step: 1 predictor + `freqiter - 1`
    /// correctors.
    pub const FREQ_ITER: usize = 2;
    /// The uniform time step used when the case supplies no `tgrid`, seconds.
    pub const UNIFORM_STEP: f64 = 0.01;
    /// Power iterations allowed in phase 2.
    pub const REEQUILIBRATE_ITER: usize = 5000;
    /// Phase-2 convergence tolerance, on both the flux and `k_eff` residuals.
    pub const REEQUILIBRATE_TOL: f64 = 1e-9;
    /// Nodal-correction refinement passes at the fixed converged flux.
    pub const NODAL_REFINE: usize = 4;
    /// The divergence guard on `P/P0`.
    ///
    /// Deliberately far above any physical excursion: an HZP case starting at
    /// `P0 ~ kW` can reach `P/P0 ~ 1e6` legitimately, so the guard only trips
    /// at `1e12`.
    pub const DIVERGENCE_CAP: f64 = 1e12;
    /// Lower clamp on the per-step exponent `omega*dt`.
    pub const OMEGA_DT_MIN: f64 = -0.9;
    /// Upper clamp on the per-step exponent `omega*dt`.
    pub const OMEGA_DT_MAX: f64 = 2.0;
}

/// `g0(x) = (exp(x) - 1 - x) / x^2`, with the series near zero.
///
/// The precursor coefficient of Eq. (9). At `x = 0` the closed form is `0/0`;
/// the limit is `1/2` and the reference switches to the series below
/// `|x| < 1e-4`. Reproduced with the same threshold and the same three terms.
fn gexp0(x: f64) -> f64 {
    if x.abs() < 1e-4 {
        0.5 + x / 6.0 + x * x / 24.0
    } else {
        (x.exp() - 1.0 - x) / (x * x)
    }
}

/// `g1(x) = (x - 1 + exp(-x)) / x^2`, with the series near zero.
///
/// The companion of [`gexp0`] from Eq. (10); same limit of `1/2`, same
/// threshold, alternating signs on the series.
fn gexp1(x: f64) -> f64 {
    if x.abs() < 1e-4 {
        0.5 - x / 6.0 + x * x / 24.0
    } else {
        (x - 1.0 + (-x).exp()) / (x * x)
    }
}

/// Per-node, per-group exponential-transform frequencies, Ants Eq. (4).
///
/// `omega = ln(phi_new / phi_old) / dt`, and **zero** wherever either flux is
/// non-positive or non-finite or the node is void — the reference guards all
/// four conditions together, so a node just emerging from zero flux
/// contributes no frequency rather than an infinite one.
fn expfreq(phinew: &[f64], phiold: &[f64], dt: f64, invv: &[f64]) -> Vec<f64> {
    phinew
        .iter()
        .zip(phiold)
        .zip(invv)
        .map(|((new, old), iv)| {
            if new.is_finite() && old.is_finite() && *new > 0.0 && *old > 0.0 && *iv > 0.0 {
                (new / old).ln() / dt
            } else {
                0.0
            }
        })
        .collect()
}

/// Why the time integration stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// The whole grid was marched.
    Completed,
    /// The divergence guard tripped; the histories are truncated at that step.
    ///
    /// The reference raises a warning and `break`s, then trims every history
    /// vector to length `n`. Same here — the truncation is real, not cosmetic,
    /// so a caller reading `time.len()` sees where it actually stopped.
    Diverged,
}

/// The NEACRP-L-335 section 4 C transient results, plus what the reference
/// writes to disk.
#[derive(Clone, Debug)]
pub struct TransientOutput {
    /// `output.k_eff` — the re-equilibrated initial eigenvalue from phase 2.
    pub k_eff: f64,
    /// The converged steady state phase 1 produced.
    pub steady: CoupledOutput,
    /// `output.th` — the final transient T-H state.
    pub th: Th,

    /// `output.time` — the time grid actually marched, seconds.
    pub time: Vec<f64>,
    /// **C1** — core power relative to its steady value.
    pub relpower: Vec<f64>,
    /// **C2** — core-averaged fuel temperature, K.
    pub avgfueltemp: Vec<f64>,
    /// **C3** — maximum fuel temperature, K.
    pub maxfueltemp: Vec<f64>,
    /// **C4** — core-averaged coolant outlet temperature, K.
    pub coolouttemp: Vec<f64>,
    /// The ejected bank's position at each step, in steps.
    pub rodpos: Vec<f64>,

    /// **C5-1** — radial power map at active layer 6, at the power maximum,
    /// normalised to a peak of 1.
    pub rad_c5_z6: Array2<f64>,
    /// **C5-2** — the same at active layer 13.
    pub rad_c5_z13: Array2<f64>,
    /// **C6-1** — radial power map at active layer 6, at `t = tend`.
    pub rad_c6_z6: Array2<f64>,
    /// **C6-2** — the same at active layer 13.
    pub rad_c6_z13: Array2<f64>,

    /// When the power maximum occurred, seconds.
    pub tpmax: f64,
    /// The peak `P/P0`.
    pub prelmax: f64,
    /// The flux at the final time.
    pub scalar_flux_final: Vec<f64>,
    /// Group-collapsed node power at the final time.
    pub pwrdens_final: Vec<f64>,
    /// Precursor concentrations at the final time, `philenf` by families.
    pub precursors_final: Array2<f64>,
    /// Which scheme ran.
    pub timescheme: TimeScheme,
    /// Why it stopped.
    pub termination: Termination,
    /// How many power iterations phase 2 needed.
    pub reequilibrate_iterations: usize,
    /// **Whether Phase-2 re-equilibration actually converged** — defect C7.
    ///
    /// The reference runs its 5000-iteration power iteration and carries on
    /// with whatever it holds when the counter runs out, with no error and no
    /// flag, so a caller cannot tell a re-equilibrated state from an abandoned
    /// one. **The iteration and its result are unchanged**; this is the
    /// verdict the reference does not record.
    ///
    /// `false` means the transient started from a state that is not a solution
    /// of the operator it is about to be marched with — which is precisely the
    /// inconsistency Phase 2 exists to remove.
    pub reequilibrate_converged: bool,
    /// The final relative fission-source residual from that loop, against a
    /// tolerance of [`defaults::REEQUILIBRATE_TOL`]. Defect C7.
    pub reequilibrate_residual: f64,
}

/// Radial power map of one **active-core** axial block.
///
/// Block `l` spans mesh layers `l*zscale ..< (l+1)*zscale` in 0-based terms —
/// the reference's `L*zsc+1 ... (L+1)*zsc`, which treats block 0 as the lower
/// reflector. See the module docs on why this over-runs a 14-layer case.
fn radial_map_layer(
    pnode: &[f64],
    l: usize,
    maxix: usize,
    maxiy: usize,
    maxiz: usize,
    zscale: usize,
) -> Array2<f64> {
    let mut p = Array2::<f64>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let mut acc = 0.0;
            for k in 0..zscale {
                let iz = l * zscale + k;
                if iz < maxiz {
                    acc += pnode[ix * maxiy * maxiz + iy * maxiz + iz];
                }
            }
            p.set(ix, iy, acc);
        }
    }
    p
}

/// Normalise a map to a peak of 1, as the reference's `P/max(P(:))` does.
///
/// A map that is entirely zero is left alone rather than turned into `NaN` —
/// the reference would divide by zero here, and case D1's "layer 13" map can
/// legitimately be empty (see the module docs).
fn normalise_peak(map: &mut Array2<f64>) {
    let peak = (0..map.rows())
        .flat_map(|i| (0..map.cols()).map(move |j| (i, j)))
        .map(|(i, j)| map.get(i, j))
        .fold(f64::NEG_INFINITY, f64::max);
    if peak > 0.0 {
        for i in 0..map.rows() {
            for j in 0..map.cols() {
                let v = map.get(i, j);
                map.set(i, j, v / peak);
            }
        }
    }
}

/// Build the time grid the reference marches.
///
/// `[0, tgrid..., tend]`, then **rounded to 1 microsecond and deduplicated**,
/// which is how the reference stops overlapping range endpoints (its cases
/// write grids like `[0:0.025:2, 2:0.05:6, ...]`, repeating every join) from
/// producing a near-zero time step. Finally anything past `tend` is dropped.
/// `build_time_grid` exposed for the case modules' tests.
///
/// The grid construction is the only part of this driver a case can get wrong
/// on its own — an overlapping range that survives deduplication would be a
/// zero-length time step — so it is worth testing from the case side.
pub fn build_time_grid_for_test(params: &Params, tend: f64) -> Vec<f64> {
    build_time_grid(params, tend)
}

fn build_time_grid(params: &Params, tend: f64) -> Vec<f64> {
    let mut grid: Vec<f64> = match &params.tgrid {
        Some(g) => {
            let mut v = Vec::with_capacity(g.len() + 2);
            v.push(0.0);
            v.extend_from_slice(g);
            v.push(tend);
            v
        }
        None => {
            let n = (tend / defaults::UNIFORM_STEP).floor() as usize;
            let mut v: Vec<f64> = (0..=n).map(|i| i as f64 * defaults::UNIFORM_STEP).collect();
            v.push(tend);
            v
        }
    };

    // Round to 1 us, deduplicate, keep only t <= tend.
    let mut ticks: Vec<i64> = grid.drain(..).map(|t| (t * 1e6).round() as i64).collect();
    ticks.sort_unstable();
    ticks.dedup();
    let cap = (tend * 1e6).round() as i64;
    ticks
        .into_iter()
        .filter(|t| *t <= cap)
        .map(|t| t as f64 / 1e6)
        .collect()
}

/// `output = thdiffusion_solvertimexyz(geometry, params, th, sigmavalues, whichsigma, varargin)`.
///
/// # Arguments
///
/// - `feedback` — the cross-section slope tables, as the steady driver takes.
/// - `initial_steady` — a precomputed phase-1 result to reuse. `None` runs
///   phase 1. This replaces the reference's `params.steadyfile` `.mat` cache;
///   see the module docs.
/// - `initial_k_eff` — passed through to phase 1.
///
/// # Errors
///
/// Propagates whatever the steady solver and the operator chain raise.
///
/// # Panics
///
/// If the case supplies neither `params.tend` nor `params.tgrid` — the
/// reference raises `thdiffusion_solvertimexyz:notimedata` here — or if the
/// kinetics data (`velocities`, `beta_dnp`, `lambda_dnp`) is missing or
/// inconsistent.
#[allow(clippy::too_many_arguments)]
pub fn thdiffusion_solvertimexyz(
    geometry: &Geometry,
    params: &Params,
    th: &Th,
    sigmavaluesref: &SigmaValues,
    feedback: &FeedbackTables,
    whichsigmaref: &Array3<usize>,
    initial_steady: Option<&CoupledOutput>,
    initial_k_eff: Option<f64>,
) -> Result<TransientOutput> {
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let g_count = params.g;
    let es = maxix * maxiy * maxiz;
    let philen = es * g_count;
    let philenf = philen + params.nc_or_zero() * es;

    // `ViG = repmat(geometry.Vi, G, 1)` — the node volume, tiled per group.
    let vig: Vec<f64> = (0..philen).map(|i| geometry.vi[i % es]).collect();

    // ----- kinetics data -----
    let v = &params.velocities;
    let beta = &params.beta_dnp;
    let lambda = &params.lambda_dnp;
    assert!(!v.is_empty(), "params.velocities must be set by the case file");
    assert_eq!(v.len(), g_count, "one velocity per energy group");
    assert_eq!(
        beta.len(),
        lambda.len(),
        "beta_dnp and lambda_dnp must have the same length"
    );
    let betatot: f64 = beta.iter().sum();
    let ndnp = beta.len();

    // ----- transient controls -----
    let tend = params
        .tend
        .or_else(|| {
            params
                .tgrid
                .as_ref()
                .and_then(|g| g.iter().cloned().fold(None, |a: Option<f64>, x| Some(a.map_or(x, |m: f64| m.max(x)))))
        })
        .expect("params.tend and/or params.tgrid must be set by the geometry case");
    let tgrid = build_time_grid(params, tend);
    let nt = tgrid.len();

    let npic = params.timepicard.unwrap_or(defaults::PICARD);
    let nodalupdtime = params.nodalupdtime.unwrap_or(defaults::NODAL_UPDATE);
    let nfreq = params.freqiter.unwrap_or(defaults::FREQ_ITER).max(1);

    // Control-assembly ejection. Optional: cases with no rod motion (the BWR
    // D1 cold-water transient) leave `crodeject` unset.
    let ejbank = geometry.crodeject;
    let ej0 = ejbank.map(|b| geometry.crod[b - 1]).unwrap_or(0.0);
    let ejto = geometry.crodejectto;
    let ejdur = params.ejectduration.unwrap_or(1.0);

    // =================================================================== //
    // Phase 1: initial steady state
    // =================================================================== //
    let steady = match initial_steady {
        Some(s) => s.clone(),
        None => thdiffusion_solverxyz(
            geometry,
            params,
            th,
            sigmavaluesref,
            feedback,
            whichsigmaref,
            initial_k_eff,
        )?,
    };

    let mut phi: Vec<f64> = (0..steady.scalar_flux.rows())
        .map(|i| steady.scalar_flux.get(i, 0))
        .collect();
    let mut th = steady.th.clone();
    let mut k0 = steady.k_eff;
    let powratio0 = th.powratio;

    // ============================================================ //
    // Phase 2: rebuild operators and re-equilibrate
    // ============================================================ //
    let mut geomt = geometry.clone(); // local copy carrying the moving CA position

    let (sigmavalues_t, whichsigma_t, _rod) =
        sigmavalupd3d_handler(params, &geomt, sigmavaluesref, feedback, whichsigmaref, &th)?;
    let mut sigma =
        crate::makesigmadfxyz::makesigmadfxyz(params, &sigmavalues_t, &whichsigma_t, None);
    let mut diffd =
        crate::calcdiffvalues3d::calcdiffvalues3d(params, &sigmavalues_t.tot, &whichsigma_t, None);
    let mut gradd = crate::makegrad_dxyz::makegrad_dxyz(&geomt, params, &diffd, &whichsigma_t, None)?;
    let mut coeffs = crate::calc_abefghxyz::calc_abefghxyz(params, &geomt, &mut sigma, &diffd);

    let mut buck_cache = crate::calc_bucklingxyz::BucklingCache::new();
    let mut nodalterms = Array2::<f64>::zeros(philen, 6);
    let mut sanodal = crate::calc_sanodalxyz::calc_sanodalxyz(
        params,
        &geomt,
        &coeffs,
        &phi,
        &mut sigma,
        &diffd,
        &gradd.terms,
        &nodalterms,
        k0,
        &mut buck_cache,
    );
    nodalterms = sanodal.terms.clone();
    // The reference then refines the correction four more times at the fixed
    // converged flux, feeding each pass's `nodalterms` into the next.
    for _ in 0..defaults::NODAL_REFINE {
        sanodal = crate::calc_sanodalxyz::calc_sanodalxyz(
            params,
            &geomt,
            &coeffs,
            &phi,
            &mut sigma,
            &diffd,
            &gradd.terms,
            &nodalterms,
            k0,
            &mut buck_cache,
        );
        nodalterms = sanodal.terms.clone();
    }

    let mut m_operator = SparseMatrix::combine(&[
        (&gradd.operator, 1.0),
        (&sanodal.operator, 1.0),
        (&sigma.tot, 1.0),
        (&sigma.s, -1.0),
    ]);

    // Short power iteration so `(phi, k0)` is an exact equilibrium of `M`.
    // Heavily rodded cores have a high dominance ratio, so the reference
    // allows many cheap triangular solves rather than exiting unconverged.
    let dm = Decomposition::new(&mut m_operator);
    let mut fs = sigma.f.mul_vec(&phi);
    let fsnorm0: f64 = fs.iter().sum();
    let mut reequilibrate_iterations = 0usize;
    // Defect C7 — see `reequilibrate_converged`.
    let mut reequilibrate_converged = false;
    let mut reequilibrate_residual = f64::INFINITY;
    for it in 1..=defaults::REEQUILIBRATE_ITER {
        reequilibrate_iterations = it;
        let rhs: Vec<f64> = fs.iter().map(|x| x / k0).collect();
        let mut phinew = crate::fixinfnan::fixinfnan(&dm.solve(&rhs), false);
        let mut fsnew = sigma.f.mul_vec(&phinew);
        let k0new = k0 * norm1(&fsnew) / norm1(&fs);
        let scale = fsnorm0 / fsnew.iter().sum::<f64>();
        for x in phinew.iter_mut() {
            *x *= scale;
        }
        for x in fsnew.iter_mut() {
            *x *= scale;
        }
        let diff: Vec<f64> = fsnew.iter().zip(&fs).map(|(a, b)| a - b).collect();
        let resid = norm2(&diff) / norm2(&fs);
        let kres = (k0new - k0).abs() / k0;
        phi = phinew;
        fs = fsnew;
        k0 = k0new;
        reequilibrate_residual = resid;
        if resid < defaults::REEQUILIBRATE_TOL && kres < defaults::REEQUILIBRATE_TOL {
            reequilibrate_converged = true;
            break;
        }
    }

    // ----- inverse velocity vector (zero on void nodes) -----
    let mut invv = vec![0.0; philenf];
    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                if whichsigmaref.get(ix, iy, iz) == 0 {
                    continue;
                }
                let idx = ix * maxiy * maxiz + iy * maxiz + iz;
                for (g, vel) in v.iter().enumerate() {
                    invv[g * es + idx] = 1.0 / vel;
                }
            }
        }
    }

    // ----- initial precursor concentrations (equilibrium) -----
    let mut c_conc = Array2::<f64>::zeros(philenf, ndnp);
    for (i, (b, l)) in beta.iter().zip(lambda).enumerate() {
        for (r, f) in fs.iter().enumerate() {
            c_conc.set(r, i, b * f / (l * k0));
        }
    }

    // ----- initial power -----
    let p0: f64 = sigma
        .fp
        .mul_vec(&phi)
        .iter()
        .zip(&vig)
        .map(|(a, b)| a * b)
        .sum();

    // ----- output bookkeeping -----
    // Fuel node mask. **Compositions 4 and above are fuel** in the NEACRP
    // composition map — a magic number in the reference, reproduced as is.
    let fuelmask: Vec<f64> = (0..es)
        .map(|idx| {
            let ix = idx / (maxiy * maxiz);
            let iy = (idx / maxiz) % maxiy;
            let iz = idx % maxiz;
            if whichsigmaref.get(ix, iy, iz) >= 4 {
                1.0
            } else {
                0.0
            }
        })
        .collect();

    // Channel outlet nodes: the top node of every fuel-bearing column.
    let mut outletidx: Vec<usize> = Vec::new();
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let col = ix * maxiy * maxiz + iy * maxiz;
            if fuelmask[col..col + maxiz].iter().any(|m| *m != 0.0) {
                let zhi = geomt
                    .zhis
                    .as_ref()
                    .map(|z| z.get(ix, iy))
                    .unwrap_or(maxiz - 1);
                outletidx.push(col + zhi);
            }
        }
    }

    // Radial volume weights for the in-rod fuel temperature average. Solution
    // ids `0..fueln` are the fuel rings up to their centres; id `fueln` is the
    // surface node covering `[Ctr(fueln-1), fuelrad]`.
    let fueln = params.fuel.fueln;
    let ctrf = &geomt.fuel.ctr;
    let rf = geomt.fuel.fuelrad;
    let mut wrad = vec![0.0; fueln + 1];
    wrad[0] = ctrf[0] * ctrf[0];
    for i in 1..fueln {
        wrad[i] = ctrf[i] * ctrf[i] - ctrf[i - 1] * ctrf[i - 1];
    }
    wrad[fueln] = rf * rf - ctrf[fueln - 1] * ctrf[fueln - 1];
    for w in wrad.iter_mut() {
        *w /= rf * rf;
    }

    let fuelvol: Vec<f64> = geomt.vi.iter().zip(&fuelmask).map(|(v, m)| v * m).collect();
    let fuelvol_total: f64 = fuelvol.iter().sum();

    let calc_avg_fuel = |t: &Th| -> f64 {
        let acc: f64 = (0..es)
            .map(|i| {
                let radial: f64 = (0..=fueln).map(|j| t.fueltemp.get(i, j) * wrad[j]).sum();
                radial * fuelvol[i]
            })
            .sum();
        acc / fuelvol_total
    };
    let calc_max_fuel = |t: &Th| -> f64 {
        (0..es)
            .filter(|i| fuelmask[*i] == 1.0)
            .flat_map(|i| (0..=fueln).map(move |j| (i, j)))
            .map(|(i, j)| t.fueltemp.get(i, j))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let calc_cool_out = |t: &Th| -> f64 {
        outletidx.iter().map(|i| t.coolant.temps[*i]).sum::<f64>() / outletidx.len() as f64
    };

    // Group-collapsed node power.
    let collapse = |pwr: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; es];
        for (i, o) in out.iter_mut().enumerate() {
            for g in 0..g_count {
                *o += pwr[g * es + i];
            }
        }
        out
    };

    // ----- time histories (sections C1-C4) -----
    let mut prel = vec![1.0; nt];
    let mut avgfueltemp = vec![0.0; nt];
    let mut maxfueltemp = vec![0.0; nt];
    let mut coolouttemp = vec![0.0; nt];
    let mut rodpos = vec![0.0; nt];

    avgfueltemp[0] = calc_avg_fuel(&th);
    maxfueltemp[0] = calc_max_fuel(&th);
    coolouttemp[0] = calc_cool_out(&th);
    rodpos[0] = ej0;

    let mut pwrnode = collapse(
        &sigma
            .fp
            .mul_vec(&phi)
            .iter()
            .zip(&vig)
            .map(|(a, b)| a * b)
            .collect::<Vec<f64>>(),
    );
    let mut pwrnode_pmax = pwrnode.clone();
    let mut prelmax = 1.0;
    let mut tpmax = 0.0;

    // ============================================================ //
    // Phase 3: time integration
    // ============================================================ //
    let mut sigmafold = sigma.f.clone(); // fission operator of the previous step (F0 terms)
    let mut termination = Termination::Completed;
    let mut last_step = nt;

    for n in 1..nt {
        let t = tgrid[n];
        let dt = t - tgrid[n - 1];

        // Prescribed CA ejection: linear over `ejdur`, then fully withdrawn.
        if let Some(bank) = ejbank {
            geomt.crod[bank - 1] = ej0 + (ejto - ej0) * (t / ejdur).min(1.0);
            rodpos[n] = geomt.crod[bank - 1];
        }

        // Time-dependent inlet forcing. The transient enthalpy march reads
        // `coolant.inlettemp` fresh each step, so overwriting it here applies
        // the new-time boundary value to the implicit step.
        if let Some(tin) = th.inlettemp_t.at(t) {
            th.coolant.inlettemp = tin;
        }

        let phiold = phi.clone();
        let cold = c_conc.clone();
        let thold = th.clone();

        // Carried out of the frequency loop for the analytic precursor update.
        let mut f1 = Array2::<f64>::zeros(philenf, ndnp);
        let mut del0 = Array2::<f64>::zeros(philenf, ndnp);

        for pic in 0..npic {
            // --- cross sections / operators at the current rod position and T-H state ---
            let (sv_t, ws_t, _rod) =
                sigmavalupd3d_handler(params, &geomt, sigmavaluesref, feedback, whichsigmaref, &th)?;
            sigma = crate::makesigmadfxyz::makesigmadfxyz(params, &sv_t, &ws_t, None);
            diffd = crate::calcdiffvalues3d::calcdiffvalues3d(params, &sv_t.tot, &ws_t, None);
            gradd = crate::makegrad_dxyz::makegrad_dxyz(&geomt, params, &diffd, &ws_t, None)?;
            coeffs = crate::calc_abefghxyz::calc_abefghxyz(params, &geomt, &mut sigma, &diffd);

            // `mod(n-2, nodalupdtime) == 0` in 1-based terms is `mod(n-1, ...)`
            // here, since our `n` is already one lower.
            if nodalupdtime > 0 && ((n - 1) % nodalupdtime == 0 || npic > 1) {
                sanodal = crate::calc_sanodalxyz::calc_sanodalxyz(
                    params,
                    &geomt,
                    &coeffs,
                    &phi,
                    &mut sigma,
                    &diffd,
                    &gradd.terms,
                    &nodalterms,
                    k0,
                    &mut buck_cache,
                );
                nodalterms = sanodal.terms.clone();
            }
            let m_step = SparseMatrix::combine(&[
                (&gradd.operator, 1.0),
                (&sanodal.operator, 1.0),
                (&sigma.tot, 1.0),
                (&sigma.s, -1.0),
            ]);

            match params.timescheme {
                TimeScheme::ExponentialTransform => {
                    for fi in 0..nfreq {
                        let (omega, omegadt) = if fi == 0 {
                            // Predictor: omega = 0, i.e. plain implicit Euler.
                            (vec![0.0; philenf], vec![0.0; philenf])
                        } else {
                            let raw = match params.freqmode {
                                FreqMode::Node => expfreq(&phi, &phiold, dt, &invv),
                                FreqMode::Global => {
                                    // Per-group GLOBAL amplitude frequencies,
                                    // uniform in space, from the
                                    // volume-integrated group flux.
                                    let mut w = vec![0.0; philenf];
                                    for g in 0..g_count {
                                        let lo = g * es;
                                        let mut num = 0.0;
                                        let mut den = 0.0;
                                        for i in 0..es {
                                            if invv[lo + i] > 0.0 {
                                                num += phi[lo + i] * vig[lo + i];
                                                den += phiold[lo + i] * vig[lo + i];
                                            }
                                        }
                                        if num.is_finite() && den.is_finite() && num > 0.0 && den > 0.0
                                        {
                                            let val = (num / den).ln() / dt;
                                            for i in 0..es {
                                                if invv[lo + i] > 0.0 {
                                                    w[lo + i] = val;
                                                }
                                            }
                                        }
                                    }
                                    w
                                }
                            };
                            // Clamp the per-step exponent. A physics bound, not
                            // overflow protection — see the module docs.
                            // `min(max(x, -0.9), 2)`, kept as a max/min chain
                            // rather than `clamp`. The two differ on NaN:
                            // MATLAB's `max(NaN, a)` is `a`, and Rust's
                            // `f64::max` agrees, so a NaN frequency is pulled
                            // to the floor — but `f64::clamp` propagates NaN.
                            // Same reasoning as elsewhere in this crate.
                            #[allow(clippy::manual_clamp)]
                            let od: Vec<f64> = raw
                                .iter()
                                .map(|x| {
                                    (x * dt)
                                        .max(defaults::OMEGA_DT_MIN)
                                        .min(defaults::OMEGA_DT_MAX)
                                })
                                .collect();
                            let w: Vec<f64> = od.iter().map(|x| x / dt).collect();
                            (w, od)
                        };

                        // Precursor coefficients, Eqs. (9)-(10) with
                        // `x = (lambda + omega)*dt`.
                        let mut ff1 = vec![0.0; philenf];
                        // V0 term, Eq. (12).
                        let mut rhs: Vec<f64> = (0..philenf)
                            .map(|r| invv[r] * omegadt[r].exp() * phiold[r] / dt)
                            .collect();
                        f1 = Array2::<f64>::zeros(philenf, ndnp);
                        del0 = Array2::<f64>::zeros(philenf, ndnp);
                        for i in 0..ndnp {
                            let f0i: Vec<f64> = (0..philenf)
                                .map(|r| {
                                    let x = (lambda[i] + omega[r]) * dt;
                                    beta[i] * dt * (-lambda[i] * dt).exp() * gexp0(x)
                                })
                                .collect();
                            let f1i: Vec<f64> = (0..philenf)
                                .map(|r| {
                                    let x = (lambda[i] + omega[r]) * dt;
                                    beta[i] * dt * gexp1(x)
                                })
                                .collect();
                            let weighted: Vec<f64> =
                                f0i.iter().zip(&phiold).map(|(a, b)| a * b).collect();
                            let d0 = sigmafold.mul_vec(&weighted);
                            for r in 0..philenf {
                                f1.set(r, i, f1i[r]);
                                del0.set(r, i, d0[r] / k0);
                                ff1[r] += lambda[i] * f1i[r];
                                rhs[r] += lambda[i]
                                    * ((-lambda[i] * dt).exp() * cold.get(r, i) + d0[r] / k0);
                            }
                        }

                        // V1 term Eq. (13): the F1 delayed production of the new
                        // flux moves into the system matrix as a column scaling
                        // of the fission operator.
                        let ff1_over_k: Vec<f64> = ff1.iter().map(|x| x / k0).collect();
                        let f_scaled = sigma.f.scale_columns(&ff1_over_k);
                        let time_term: Vec<f64> = (0..philenf)
                            .map(|r| invv[r] * (omega[r] + 1.0 / dt))
                            .collect();
                        let mut lhs = SparseMatrix::combine(&[
                            (&SparseMatrix::from_diagonal(&time_term), 1.0),
                            (&m_step, 1.0),
                            (&sigma.f, -(1.0 - betatot) / k0),
                            (&f_scaled, -1.0),
                        ]);
                        let dlhs = Decomposition::new(&mut lhs);
                        phi = crate::fixinfnan::fixinfnan(&dlhs.solve(&rhs), false);
                    }

                    // --- analytic precursor update, Eq. (8) ---
                    for (i, l) in lambda.iter().enumerate() {
                        let weighted: Vec<f64> =
                            (0..philenf).map(|r| f1.get(r, i) * phi[r]).collect();
                        let prod = sigma.f.mul_vec(&weighted);
                        let decay = (-l * dt).exp();
                        for (r, pr) in prod.iter().enumerate() {
                            c_conc.set(
                                r,
                                i,
                                decay * cold.get(r, i) + del0.get(r, i) + pr / k0,
                            );
                        }
                    }
                }
                TimeScheme::ImplicitEuler => {
                    // Plain implicit Euler with the precursors eliminated.
                    let wdel: f64 = beta
                        .iter()
                        .zip(lambda)
                        .map(|(b, l)| b * l * dt / (1.0 + l * dt))
                        .sum();
                    let time_term: Vec<f64> = invv.iter().map(|x| x / dt).collect();
                    let mut lhs = SparseMatrix::combine(&[
                        (&SparseMatrix::from_diagonal(&time_term), 1.0),
                        (&m_step, 1.0),
                        (&sigma.f, -((1.0 - betatot) + wdel) / k0),
                    ]);
                    let mut rhs: Vec<f64> =
                        (0..philenf).map(|r| invv[r] * phiold[r] / dt).collect();
                    for (i, l) in lambda.iter().enumerate() {
                        let w = l / (1.0 + l * dt);
                        for (r, x) in rhs.iter_mut().enumerate() {
                            *x += w * cold.get(r, i);
                        }
                    }
                    let dlhs = Decomposition::new(&mut lhs);
                    phi = crate::fixinfnan::fixinfnan(&dlhs.solve(&rhs), false);

                    // --- implicit Euler precursor update ---
                    let fsnew = sigma.f.mul_vec(&phi);
                    for (i, (b, l)) in beta.iter().zip(lambda).enumerate() {
                        for (r, f) in fsnew.iter().enumerate() {
                            c_conc.set(
                                r,
                                i,
                                (cold.get(r, i) + dt * b * f / k0) / (1.0 + l * dt),
                            );
                        }
                    }
                }
            }

            // --- transient T-H step ---
            let pwr: Vec<f64> = sigma
                .fp
                .mul_vec(&phi)
                .iter()
                .zip(&vig)
                .map(|(a, b)| a * b)
                .collect();
            let prelt: f64 = pwr.iter().sum::<f64>() / p0;
            th.powratio = powratio0 * prelt;
            let (thnew, _rods) = crate::th_solvertimexyz::th_solvertimexyz(
                params, &geomt, &th, &ws_t, &pwr, &thold, dt,
            );
            th = thnew;
            let _ = pic;
        }

        // Lagged fission operator for the next step's F0 terms.
        sigmafold = sigma.f.clone();

        // --- record histories ---
        let pwr: Vec<f64> = sigma
            .fp
            .mul_vec(&phi)
            .iter()
            .zip(&vig)
            .map(|(a, b)| a * b)
            .collect();
        let prelt: f64 = pwr.iter().sum::<f64>() / p0;
        prel[n] = prelt;
        avgfueltemp[n] = calc_avg_fuel(&th);
        maxfueltemp[n] = calc_max_fuel(&th);
        coolouttemp[n] = calc_cool_out(&th);

        pwrnode = collapse(&pwr);
        if prelt > prelmax {
            prelmax = prelt;
            tpmax = t;
            pwrnode_pmax = pwrnode.clone();
        }

        // Divergence guard: stop rather than march a blown-up solution to the
        // end. The cap sits far above physical HZP excursions.
        if !prelt.is_finite() || !(0.0..=defaults::DIVERGENCE_CAP).contains(&prelt) {
            termination = Termination::Diverged;
            last_step = n + 1;
            break;
        }
    }

    // Truncate every history to where the march actually stopped.
    let nt_final = if termination == Termination::Diverged {
        last_step
    } else {
        nt
    };
    let tgrid_out = tgrid[..nt_final].to_vec();
    prel.truncate(nt_final);
    avgfueltemp.truncate(nt_final);
    maxfueltemp.truncate(nt_final);
    coolouttemp.truncate(nt_final);
    rodpos.truncate(nt_final);

    // ============================================================ //
    // Outputs (NEACRP-L-335 section 4 C)
    // ============================================================ //
    let pwrnode_final = pwrnode;
    // `.max(1)` guards a case that never set `zscale`; a zero would collapse
    // every radial map to nothing. The cases in the snapshot all set it.
    let zsc = geomt.zscale.max(1);

    let mut rad_c5_z6 = radial_map_layer(&pwrnode_pmax, 6, maxix, maxiy, maxiz, zsc);
    let mut rad_c5_z13 = radial_map_layer(&pwrnode_pmax, 13, maxix, maxiy, maxiz, zsc);
    let mut rad_c6_z6 = radial_map_layer(&pwrnode_final, 6, maxix, maxiy, maxiz, zsc);
    let mut rad_c6_z13 = radial_map_layer(&pwrnode_final, 13, maxix, maxiy, maxiz, zsc);
    normalise_peak(&mut rad_c5_z6);
    normalise_peak(&mut rad_c5_z13);
    normalise_peak(&mut rad_c6_z6);
    normalise_peak(&mut rad_c6_z13);

    Ok(TransientOutput {
        k_eff: k0,
        steady,
        th,
        time: tgrid_out,
        relpower: prel,
        avgfueltemp,
        maxfueltemp,
        coolouttemp,
        rodpos,
        rad_c5_z6,
        rad_c5_z13,
        rad_c6_z6,
        rad_c6_z13,
        tpmax,
        prelmax,
        scalar_flux_final: phi,
        pwrdens_final: pwrnode_final,
        precursors_final: c_conc,
        timescheme: params.timescheme,
        termination,
        reequilibrate_iterations,
        reequilibrate_converged,
        reequilibrate_residual,
    })
}
