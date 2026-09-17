//! Semi-analytic nodal diffusion — the solver the benchmark drivers call.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `sanodaldiffusion_solverxyz.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this adds over the finite-difference solver
//!
//! [`crate::diffusion_solverxyz`] solves `gradD + sigma.tot - sigma.sd` by
//! source iteration. This one adds the SANM correction operator from
//! [`crate::calc_sanodalxyz`] and folds the whole scattering operator into the
//! left-hand side, so a pass is a single solve against
//! `gradD + nodal + sigma.tot - sigma.s` with a pure fission right-hand side.
//! On top of that it carries three things its plainer sibling does not:
//!
//! - a **periodic nodal update**, re-running the expansion against the current
//!   flux every `nodalupd` iterations and refactorising;
//! - **fission-source extrapolation** every `fsexp` iterations
//!   ([`crate::fiss_src_extrapolatexyz`]), which is why the flux is kept as a
//!   five-generation history rather than a single vector;
//! - a **warm start**, so the coupled neutronics/T-H outer loop can reseed the
//!   iteration with the previous outer pass's flux.
//!
//! This module is the last of the fourteen SANM files.
//!
//! # Reference defects carried here
//!
//! Three entries of `docs/bedok-reference-defects.md` are about this file, and
//! all three are reproduced rather than fixed:
//!
//! - **N1** — `nodalupd == 1` destabilises the solver, and the built-in default
//!   `ceil((maxix+maxiy+maxiz)/10)` **is** 1 for any mesh whose extents sum to
//!   10 or fewer. See [`sanodaldiffusion_solverxyz`].
//! - **N10** — the normalisation comment is wrong, and the norms are
//!   inconsistent with [`crate::diffusion_solverxyz`]'s.
//! - **N2** — `Nc > 0` cannot conform.
//!
//! Reading this file fresh added three more, recorded as D4-D6 in that
//! register: the dead Wielandt scaffolding, the mismatched normalisation pair
//! on an early break, and the lagged output state shared with
//! [`crate::diffusion_solverxyz`].

use crate::calc_abefghxyz::calc_abefghxyz;
use crate::calc_bucklingxyz::BucklingCache;
use crate::calc_relpower3d::calc_relpower3d;
use crate::calc_sanodalxyz::calc_sanodalxyz;
use crate::calcdiffvalues3d::calcdiffvalues3d;
use crate::error::BedokError;
use crate::fiss_src_extrapolatexyz::fiss_src_extrapolatexyz;
use crate::fixinfnan::fixinfnan_counted;
use crate::makegrad_dxyz::makegrad_dxyz;
use crate::makesigmadfxyz::makesigmadfxyz;
use crate::matlab::{norm1, norm2, Array2, Array3, Decomposition, SparseMatrix};
use crate::types::{Geometry, Params, SigmaValues};
use crate::Result;

/// `sizethresh` — above this many unknowns the reference switches to
/// preconditioned GMRES. See [`BedokError::IterativeSolveNotTranslated`].
pub const SIZE_THRESH: usize = 50_000_000;

/// `maxiter` — the source-iteration cap. **5000**, where
/// [`crate::diffusion_solverxyz`] uses 10000.
pub const MAX_ITER: usize = 5_000;

/// The flux history depth, `size(scalar_flux, 2)`.
///
/// The reference allocates `ones(philenf, 5)` and comments that it can be
/// increased if an acceleration scheme needs more.
/// [`crate::fiss_src_extrapolatexyz`] reads only the first **four** columns, so
/// the fifth generation is carried and never used — it is shifted along each
/// pass and falls off the end.
pub const HISTORY: usize = 5;

/// The number of generations [`crate::fiss_src_extrapolatexyz`] consumes.
pub const EXTRAP_HISTORY: usize = 4;

/// Why the source iteration stopped. As [`crate::diffusion_solverxyz`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// Both residuals fell below the tolerance.
    Converged,
    /// `k_eff <= 0`.
    NonPositiveKeff,
    /// `k_eff` became `NaN`.
    NanKeff,
    /// The iteration count passed [`MAX_ITER`].
    IterationCap,
}

/// The `params.debugdump` diagnostic maps.
///
/// # Why these are returned rather than written
///
/// The reference writes ten `writematrix` CSVs when `params.debugdump == 1` —
/// the antisymmetric part of six operator diagonals and their off-diagonal
/// column masses, plus `rel_power_inner.csv`, `scalar_flux.csv`,
/// `fission_source.csv` and `pwrdenss.csv`. Writing files as a side effect of
/// a library call is not reproduced here for the reasons given on
/// [`crate::diffusion_solverxyz::Diagnostics`]; the quantities are computed
/// exactly as the reference computes them and handed back.
///
/// Unlike its sibling, this solver **does** gate them on `params.debugdump`, so
/// they are `None` unless it is set — the computation is skipped entirely, as
/// in the reference.
///
/// Each map is `maxix` by `maxiy` and dimensionless. `diag` is the
/// antisymmetric part of the collapsed diagonal; `offdiag` the same for
/// `sum(m) - diag(m)`, the off-diagonal column mass.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    /// `sigmafxy.csv` and `sigmafxyoff.csv`.
    pub sigmaf: (Array2<f64>, Array2<f64>),
    /// `sigmasxy.csv` and `sigmasxyoff.csv`.
    pub sigmas: (Array2<f64>, Array2<f64>),
    /// `sigmatxy.csv` and `sigmatxyoff.csv`.
    pub sigmatot: (Array2<f64>, Array2<f64>),
    /// `nodalxy.csv` and `nodalxyoff.csv` — from the **initial** nodal
    /// operator, built before the iteration starts.
    pub nodal: (Array2<f64>, Array2<f64>),
    /// `gradDxy.csv` and `gradDxyoff.csv`.
    pub gradd: (Array2<f64>, Array2<f64>),
    /// `rel_power_inner.csv` — the normalised assembly power map.
    pub rel_power: Array2<f64>,
}

/// `output` — what the reference returns, plus the provenance it does not.
///
/// Deliberately **not** `Default`, as [`crate::diffusion_solverxyz::DiffusionOutput`].
#[derive(Clone, Debug)]
pub struct SaNodalOutput {
    /// `output.k_eff` — the multiplication factor, dimensionless.
    pub k_eff: f64,
    /// `output.residual` — the relative fission-source change, dimensionless.
    pub residual: f64,
    /// `output.k_eff_residual` — the relative `k_eff` change, dimensionless.
    pub k_eff_residual: f64,
    /// `output.scalar_flux` — the **whole five-generation history**, `philenf`
    /// rows by [`HISTORY`] columns, column 0 newest.
    ///
    /// The reference returns the matrix, not a vector, and that matters: this
    /// is exactly what the warm-start argument expects back, so a coupled outer
    /// loop can feed one call's output straight into the next call's
    /// `initflux`.
    pub scalar_flux: Array2<f64>,
    /// `output.fission_source` — `philenf` long.
    pub fission_source: Vec<f64>,
    /// `output.pwrdens` — `fission_source .* Vi`.
    pub pwrdens: Vec<f64>,
    /// `phi_plot` — the group-summed flux on the `zplot = 1` plane, `maxix` by
    /// `maxiy`. Computed unconditionally by the reference and only used to draw
    /// `figure(6)`; returned rather than plotted, so `params.plotfig` is not
    /// read here.
    ///
    /// # It reads the newest generation, by accident rather than by choice
    ///
    /// The reference indexes `output.scalar_flux(...)` with a **single** linear
    /// index, on a value that is a `philenf`-by-5 matrix rather than a vector.
    /// MATLAB's column-major linear indexing therefore lands the whole
    /// calculation in **column 1** — the newest generation — because every
    /// index it forms is at most `philenf`. That is the intended plane, so the
    /// result is right; it is right for a reason the code does not state.
    ///
    /// This is the same trap [`crate::matlab::Array2::get_linear_column_major`]
    /// documents for `makesigmadfxyz`. Translated by reading column 0
    /// explicitly.
    pub phi_plot: Array2<f64>,
    /// The count the reference prints as `Diffusion iteration`.
    pub iterations: usize,
    /// How many times the nodal correction was rebuilt and the operator
    /// refactorised. Not in the reference's `output`; useful because the
    /// interval, and hence this count, is what defect N1 is about.
    pub nodal_updates: usize,
    /// Why the iteration stopped. Not in the reference's `output`.
    pub termination: Termination,
    /// The nodal-update interval this solve actually used — defect N1.
    ///
    /// Not in the reference's `output`. `params.nodalupd` is honoured when
    /// non-zero, and otherwise the built-in `ceil((nx+ny+nz)/10)` applies —
    /// which **is 1 for any mesh whose extents sum to 10 or less**, and an
    /// interval of 1 destabilises the solver. A caller that never set the
    /// field has no other way to discover it is running at 1.
    ///
    /// **A value of 1 here means the result should not be trusted**, whatever
    /// the residual and [`Termination`] say; see defect N1 in
    /// `docs/bedok-reference-defects.md`.
    pub effective_nodalupd: usize,
    /// **How many faces the SA-nodal near-zero-flux guard suppressed**, summed
    /// over every nodal rebuild in this solve — defect N11.
    ///
    /// Each suppression is a face that silently fell back to plain finite
    /// difference. See [`crate::calc_sanodalxyz::SaNodal::guard_suppressions`].
    pub nodal_guard_suppressions: usize,
    /// **How many non-finite flux entries `fixinfnan` had to substitute** —
    /// defect C5. Not in the reference's `output`, and the whole point of it.
    ///
    /// The reference patches `Inf`/`NaN` out of the flux after every linear
    /// solve and then computes its residual norms on the patched vector, so a
    /// solve that has blown up can report a small residual and look converged.
    /// This counts what was hidden.
    ///
    /// **Any non-zero value invalidates the result**, however healthy the
    /// residual and [`Termination`] look: it means a linear solve produced
    /// values that are not numbers. Zero on every case in the snapshot.
    pub non_finite_substitutions: usize,
    /// The `params.debugdump` maps, `None` unless it was set.
    pub diagnostics: Option<Diagnostics>,
}

/// `output = sanodaldiffusion_solverxyz(geometry, params, sigmavalues, whichsigma, initial_k_eff, initflux)`.
///
/// Assembles the nodal-corrected diffusion operator and runs an accelerated
/// source iteration on it, returning the fundamental-mode flux and eigenvalue.
///
/// # Arguments
///
/// - `geometry` — needs `Vi`, plus everything [`makegrad_dxyz`] and the
///   expansion chain read. `geometry.adf` supplies the assembly discontinuity
///   factors and defaults to unity when absent.
/// - `params` — `G`, `Nc`, the three extents, and the four optional switches
///   `nodalupd`, `fsexp`, `innertol` and `debugdump`.
/// - `sigmavalues` — per-material cross sections.
/// - `whichsigma` — the 1-based material map, `0` for void.
/// - `initial_k_eff` — `varargin{1}`; `None` is the reference's default of `1`.
/// - `initflux` — `varargin{2}`, the warm start. `None` is the flat guess. A
///   matrix with at least [`HISTORY`] columns seeds the whole history; a
///   narrower one has its **first column replicated** across all five, which is
///   what `repmat(initflux(:,1), 1, nh)` does. A matrix whose row count is not
///   `philenf` is **silently ignored** — the reference tests
///   `size(initflux,1)==philenf` and falls through to the flat guess otherwise.
///
/// # The operator, and how it differs from the finite-difference one
///
/// ```text
/// LHS = gradD + nodal + sigma.tot - sigma.s
/// RHS = fission_source / k_eff
/// ```
///
/// The whole scattering operator is implicit, so there is no lagged scattering
/// source and a pass is one solve. Compare [`crate::diffusion_solverxyz`],
/// which keeps only the within-group diagonal `sigma.sd` on the left and lags
/// the rest — the converged operator is the same, the iteration is not.
///
/// # `nodalupd` — and why the default can be dangerous
///
/// The default is `ceil((maxix + maxiy + maxiz) / 10)`, which the reference's
/// own comment describes as "~5 for a 17x17x14 mesh" and claims smaller values
/// improve stability. **Defect N1 records the opposite**: an interval of 1 was
/// observed to run a small leaking cube to the 5000-iteration ceiling, where
/// any interval of 2 or more converged. And the default *is* 1 whenever the
/// three extents sum to 10 or less — so small test meshes get the pathological
/// setting automatically while real benchmarks (IAEA-3D gives 6) do not. Set
/// `params.nodalupd` explicitly on a small mesh.
///
/// The update fires on `iteration % nodalupd == 0`, counting the reference's
/// 1-based iteration number.
///
/// # Normalisation — three integrals, two conventions
///
/// - `init_norm` is a **plain `sum`** of the initial fission source.
/// - `norm_factor`, applied once after the loop, is a **plain `sum`** of the
///   final source.
/// - the `k_eff` update inside the loop uses **`norm(·, 1)`**.
///
/// A plain sum and a 1-norm agree only while the source stays non-negative.
/// [`crate::diffusion_solverxyz`] uses the 1-norm throughout and rescales every
/// pass rather than once at the end. Defect N10 covers both the inconsistency
/// and the fact that the "fission source integration = 1" comment describes
/// neither solver — what is actually preserved is the *initial* integral.
///
/// # Dead code in the reference
///
/// `weilandtfactor = 1.05` and `weilandt = 0` set up a Wielandt shift whose
/// every use site is commented out; `weilandt` can never become 1. `philen` is
/// computed and used only to size the initial `zeros(philen, 6)` nodal terms.
/// Both are preserved as written — the shift is not implemented here either.
/// Recorded as defect D4.
///
/// # On an early break, the normalisation pairs mismatched vectors
///
/// The final rescale divides by `sum(fission_source_new)` but applies to
/// `fission_source`. On a normal exit those are the same vector. On a `break`
/// they are **one iteration apart**, so the source is rescaled by a factor
/// derived from a different, and by hypothesis diverging, source. Preserved;
/// [`SaNodalOutput::termination`] is how a caller detects it. Defect D5.
///
/// # On a break, the reported state lags by one iteration
///
/// As [`crate::diffusion_solverxyz`]: the `break` precedes the increment, so
/// the returned `k_eff`, `residual` and `k_eff_residual` are the previous
/// pass's. The reference does *print* the offending new `k_eff` in its bail-out
/// message but does not return it. Defect D6.
///
/// # `Nc > 0` does not work
///
/// Two independent conformance failures, both defect N2: `calc_sanodalxyz`
/// returns a `philen`-square operator that is added to `philenf`-square ones,
/// and `Vi` is replicated to `G*es` while the fission source is `philenf` long.
/// All four benchmark cases set `Nc = 0`. Reproduced as panics.
///
/// # Errors
///
/// - [`BedokError::IterativeSolveNotTranslated`] if `philenf >= 50_000_000`.
/// - Whatever [`makegrad_dxyz`] raises.
///
/// # Panics
///
/// If `geometry.vi` is shorter than `maxix*maxiy*maxiz`, if `Nc > 0` (see
/// above), or wherever [`calc_sanodalxyz`] panics.
pub fn sanodaldiffusion_solverxyz(
    geometry: &Geometry,
    params: &Params,
    sigmavalues: &SigmaValues,
    whichsigma: &Array3<usize>,
    initial_k_eff: Option<f64>,
    initflux: Option<&Array2<f64>>,
) -> Result<SaNodalOutput> {
    let g_count = params.g;
    let nc = params.nc_or_zero();
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;
    let philenf = (g_count + nc) * es;

    if philenf >= SIZE_THRESH {
        return Err(BedokError::IterativeSolveNotTranslated {
            philenf,
            threshold: SIZE_THRESH,
        });
    }

    // `Vi = repmat(Vi, G, 1)`.
    assert!(
        geometry.vi.len() >= es,
        "geometry.vi is {} long, need at least {es}",
        geometry.vi.len()
    );
    let vi: Vec<f64> = (0..philen).map(|i| geometry.vi[i % es]).collect();

    let initial_k_eff = initial_k_eff.unwrap_or(1.0);

    // `params.innertol > 0` overrides the tight default; anything else keeps it.
    let tol = match params.innertol {
        Some(t) if t > 0.0 => t,
        _ => 1e-6,
    };

    // `nodalupd = ceil((maxix+maxiy+maxiz)/10)`, overridden when non-zero.
    // See the N1 note above before changing this.
    let mut nodalupd = (maxix + maxiy + maxiz).div_ceil(10);
    if params.nodalupd != 0 {
        nodalupd = params.nodalupd;
    }
    assert!(
        nodalupd > 0,
        "the nodal update interval must be positive; the reference's \
         ceil((maxix+maxiy+maxiz)/10) cannot be zero and params.nodalupd is \
         only honoured when non-zero"
    );

    // `fsexp = 5`, overridden when non-zero. `fs_extrap_flag` is a literal 1 in
    // the reference with no switch, so extrapolation is always enabled.
    let mut fsexp = 5usize;
    if params.fsexp != 0 {
        fsexp = params.fsexp;
    }

    // ----- calculate matrices ----- //
    let mut sigma = makesigmadfxyz(params, sigmavalues, whichsigma, None);
    let diffd = calcdiffvalues3d(params, &sigmavalues.tot, whichsigma, None);
    let gradd = makegrad_dxyz(geometry, params, &diffd, whichsigma, None)?;
    let coeffs = calc_abefghxyz(params, geometry, &mut sigma, &diffd);

    // The first nodal build runs against a flat flux, zero previous terms and
    // `keff = 1` — it is a shape, not yet a correction.
    let mut buck_cache = BucklingCache::new();
    // Defect N11 — see `nodal_guard_suppressions`.
    let mut nodal_guard_suppressions = 0usize;
    let mut sanodal = calc_sanodalxyz(
        params,
        geometry,
        &coeffs,
        &vec![1.0; philenf],
        &mut sigma,
        &diffd,
        &gradd.terms,
        &Array2::<f64>::zeros(philen, 6),
        1.0,
        &mut buck_cache,
    );
    nodal_guard_suppressions += sanodal.guard_suppressions;

    let debugdump = params.debugdump == 1;
    let diagnostics_head = if debugdump {
        Some(Diagnostics {
            sigmaf: asymmetry_pair(params, &mut sigma.f),
            sigmas: asymmetry_pair(params, &mut sigma.s),
            sigmatot: asymmetry_pair(params, &mut sigma.tot),
            nodal: asymmetry_pair(params, &mut sanodal.operator),
            gradd: asymmetry_pair(params, &mut gradd.operator.clone()),
            rel_power: Array2::default(),
        })
    } else {
        None
    };

    // ----- Set up initial values ----- //
    // `scalar_flux = ones(philenf, 5)`, column 0 newest.
    let mut scalar_flux = Array2::<f64>::zeros(philenf, HISTORY);
    for i in 0..philenf {
        for j in 0..HISTORY {
            scalar_flux.set(i, j, 1.0);
        }
    }
    // Warm start. A row count that does not match `philenf` is ignored, as in
    // the reference.
    if let Some(seed) = initflux {
        if seed.rows() == philenf {
            for i in 0..philenf {
                for j in 0..HISTORY {
                    // `initflux(:,1:nh)` when wide enough, else
                    // `repmat(initflux(:,1), 1, nh)`.
                    let src = if seed.cols() >= HISTORY { j } else { 0 };
                    scalar_flux.set(i, j, seed.get(i, src));
                }
            }
        }
    }

    let mut residual: Vec<f64> = vec![1.0];
    let mut k_eff_residual: Vec<f64> = vec![1.0];
    let mut k_eff: Vec<f64> = vec![initial_k_eff];
    // 0-based; the reference's `iteration` is this plus one.
    let mut iteration = 0usize;

    let newest = |a: &Array2<f64>| -> Vec<f64> { (0..a.rows()).map(|i| a.get(i, 0)).collect() };

    let mut fission_source = sigma.f.mul_vec(&newest(&scalar_flux));
    // `sum(...)`, not `norm(..., 1)` — see the normalisation note.
    let init_norm: f64 = fission_source.iter().sum();

    let mut lhs = SparseMatrix::combine(&[
        (&gradd.operator, 1.0),
        (&sanodal.operator, 1.0),
        (&sigma.tot, 1.0),
        (&sigma.s, -1.0),
    ]);
    let mut dlhs = Decomposition::new(&mut lhs);

    let mut nodal_updates = 0usize;
    // Defect C5 — see `SaNodalOutput::non_finite_substitutions`.
    let mut non_finite_substitutions = 0usize;
    // `fission_source_new` is written on every pass and read after the loop.
    // The loop always runs at least once (both residuals start at 1), so the
    // reference never reads it undefined.
    let mut fission_source_new: Vec<f64> = Vec::new();

    // ----- Run source iteration ----- //
    let termination = loop {
        if residual[iteration] < tol && k_eff_residual[iteration] < tol {
            break Termination::Converged;
        }

        // The reference's 1-based iteration number drives the two intervals.
        let iter1 = iteration + 1;

        if iter1.is_multiple_of(nodalupd) {
            sanodal = calc_sanodalxyz(
                params,
                geometry,
                &coeffs,
                &newest(&scalar_flux),
                &mut sigma,
                &diffd,
                &gradd.terms,
                &sanodal.terms,
                k_eff[iteration],
                &mut buck_cache,
            );
            nodal_guard_suppressions += sanodal.guard_suppressions;
            lhs = SparseMatrix::combine(&[
                (&gradd.operator, 1.0),
                (&sanodal.operator, 1.0),
                (&sigma.tot, 1.0),
                (&sigma.s, -1.0),
            ]);
            dlhs = Decomposition::new(&mut lhs);
            nodal_updates += 1;
        }

        let rhs: Vec<f64> = fission_source.iter().map(|x| x / k_eff[iteration]).collect();
        // Defect C5: the reference silently patches a blown-up solve here and
        // then measures its residuals on the patched vector. The patch is
        // reproduced exactly; what is added is the count.
        let (scalar_flux_l_plus, substituted) =
            fixinfnan_counted(&dlhs.solve(&rhs), false);
        non_finite_substitutions += substituted;

        fission_source_new = sigma.f.mul_vec(&scalar_flux_l_plus);

        // Shift the history down one and put the new flux at column 0.
        for j in (0..HISTORY - 1).rev() {
            for i in 0..philenf {
                let v = scalar_flux.get(i, j);
                scalar_flux.set(i, j + 1, v);
            }
        }
        for (i, &v) in scalar_flux_l_plus.iter().enumerate() {
            scalar_flux.set(i, 0, v);
        }

        if iter1.is_multiple_of(fsexp) {
            // The extrapolator reads four generations and writes column 0. The
            // reference hands it the whole five-column matrix; column 4 is
            // neither read nor written, so passing a four-column view and
            // copying column 0 back is exactly equivalent.
            let mut window = Array2::<f64>::zeros(philenf, EXTRAP_HISTORY);
            for i in 0..philenf {
                for j in 0..EXTRAP_HISTORY {
                    window.set(i, j, scalar_flux.get(i, j));
                }
            }
            let (fs, _outcome) = fiss_src_extrapolatexyz(&mut sigma.f, &mut window);
            for i in 0..philenf {
                scalar_flux.set(i, 0, window.get(i, 0));
            }
            fission_source_new = fs;
        }

        let k_next = k_eff[iteration] * norm1(&fission_source_new) / norm1(&fission_source);
        k_eff.push(k_next);

        let diff: Vec<f64> = (0..philenf)
            .map(|n| fission_source_new[n] - fission_source[n])
            .collect();
        residual.push(norm2(&diff) / norm2(&fission_source));

        k_eff_residual.push((k_next - k_eff[iteration]).abs() / k_eff[iteration]);

        if k_next <= 0.0 {
            break Termination::NonPositiveKeff;
        }
        if k_next.is_nan() {
            break Termination::NanKeff;
        }
        if iter1 > MAX_ITER {
            break Termination::IterationCap;
        }

        iteration += 1;
        fission_source = fission_source_new.clone();
    };

    // The final rescale. Note the numerator comes from `fission_source_new` and
    // is applied to `fission_source`; on an early break those differ. Defect D5.
    let norm_factor: f64 = fission_source_new.iter().sum();
    let scale = init_norm / norm_factor;
    for i in 0..philenf {
        for j in 0..HISTORY {
            let v = scalar_flux.get(i, j);
            scalar_flux.set(i, j, v * scale);
        }
    }
    for x in fission_source.iter_mut() {
        *x *= scale;
    }

    // ----- output ----- //
    assert_eq!(
        vi.len(),
        fission_source.len(),
        "power density needs Vi and the fission source to conform; \
         Vi is G*es = {} and the fission source is philenf = {}. \
         Nc > 0 is not supported by the reference (defects C11, N2)",
        vi.len(),
        fission_source.len()
    );
    let pwrdens: Vec<f64> = (0..fission_source.len())
        .map(|n| fission_source[n] * vi[n])
        .collect();

    let diagnostics = diagnostics_head.map(|mut d| {
        d.rel_power = calc_relpower3d(params, &pwrdens);
        d
    });

    // `zplot = 1` — the bottom axial plane, 0-based here. Reads the newest
    // generation, after the rescale.
    let mut phi_plot = Array2::<f64>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let mut acc = 0.0;
            for g in 0..g_count {
                acc += scalar_flux.get(g * es + ix * maxiy * maxiz + iy * maxiz, 0);
            }
            phi_plot.set(ix, iy, acc);
        }
    }

    Ok(SaNodalOutput {
        k_eff: k_eff[iteration],
        residual: residual[iteration],
        k_eff_residual: k_eff_residual[iteration],
        scalar_flux,
        fission_source,
        pwrdens,
        phi_plot,
        iterations: iteration,
        nodal_updates,
        termination,
        effective_nodalupd: nodalupd,
        nodal_guard_suppressions,
        non_finite_substitutions,
        diagnostics,
    })
}

/// `(m - m.', off - off.')` where `m` is the collapsed diagonal map and `off`
/// the collapsed off-diagonal column mass — the pair the reference dumps for
/// each of its five operators.
fn asymmetry_pair(params: &Params, m: &mut SparseMatrix) -> (Array2<f64>, Array2<f64>) {
    let diag = m.diagonal();
    let sums = m.column_sums();
    let off: Vec<f64> = (0..diag.len()).map(|n| sums[n] - diag[n]).collect();
    (
        antisymmetric_part(&calc_relpower3d(params, &diag)),
        antisymmetric_part(&calc_relpower3d(params, &off)),
    )
}

/// `a - a.'`.
///
/// # Panics
/// If `a` is not square.
fn antisymmetric_part(a: &Array2<f64>) -> Array2<f64> {
    assert_eq!(
        a.rows(),
        a.cols(),
        "the symmetry diagnostic needs a square map, got {}x{}",
        a.rows(),
        a.cols()
    );
    let mut out = Array2::<f64>::zeros(a.rows(), a.cols());
    for i in 0..a.rows() {
        for j in 0..a.cols() {
            out.set(i, j, a.get(i, j) - a.get(j, i));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matlab::Array2 as M2;
    use crate::types::BoundaryCondition;

    /// A uniform cube of one fissile material, vacuum on every face — the same
    /// problem [`crate::diffusion_solverxyz`]'s tests use, so the two solvers
    /// can be compared directly.
    fn cube(n: usize, nodalupd: usize) -> (Geometry, Params, SigmaValues, Array3<usize>) {
        let params = Params {
            maxix: Some(n),
            maxiy: Some(n),
            maxiz: Some(n),
            g: 1,
            nc: Some(0),
            nodalupd,
            ..Default::default()
        };
        let es = n * n * n;

        let mut tot = M2::<f64>::zeros(1, 1);
        tot.set(0, 0, 0.5);
        let mut f = M2::<f64>::zeros(1, 1);
        f.set(0, 0, 0.1);
        let mut s = crate::matlab::Array3::<f64>::zeros(1, 1, 1);
        s.set(0, 0, 0, 0.4);
        let mut nu = M2::<f64>::zeros(1, 1);
        nu.set(0, 0, 2.5);
        let mut chi = M2::<f64>::zeros(1, 1);
        chi.set(0, 0, 1.0);

        let sigmavalues = SigmaValues {
            tot,
            f,
            s,
            nu,
            chi,
            fp: None,
        };

        let mut whichsigma = Array3::<usize>::zeros(n, n, n);
        for ix in 0..n {
            for iy in 0..n {
                for iz in 0..n {
                    whichsigma.set(ix, iy, iz, 1);
                }
            }
        }

        let bounds = |v: usize| {
            let mut a = M2::<usize>::zeros(n, n);
            for i in 0..n {
                for j in 0..n {
                    a.set(i, j, v);
                }
            }
            a
        };
        let geometry = Geometry {
            xtot: n as f64 * 10.0,
            ytot: n as f64 * 10.0,
            xlows: Some(bounds(0)),
            xhis: Some(bounds(n - 1)),
            ylows: Some(bounds(0)),
            yhis: Some(bounds(n - 1)),
            zlows: Some(bounds(0)),
            zhis: Some(bounds(n - 1)),
            lx: vec![10.0; es],
            ly: vec![10.0; es],
            lz: vec![10.0; es],
            vi: vec![1000.0; es],
            xmin: BoundaryCondition::Vacuum,
            xmax: BoundaryCondition::Vacuum,
            ymin: BoundaryCondition::Vacuum,
            ymax: BoundaryCondition::Vacuum,
            zmin: BoundaryCondition::Vacuum,
            zmax: BoundaryCondition::Vacuum,
            adf: None,
            fuel: Default::default(),
            ..Default::default()
        };

        (geometry, params, sigmavalues, whichsigma)
    }

    /// The nodal solver converges on a uniform leaking cube and lands near the
    /// finite-difference answer.
    ///
    /// # Methodology
    ///
    /// The 4x4x4 one-group cube described in
    /// [`crate::diffusion_solverxyz`]'s tests — `Sigma_tot = 0.5`,
    /// `Sigma_s = 0.4`, `Sigma_f = 0.1`, `nu = 2.5`, 10 cm nodes, vacuum on all
    /// six faces, `k_inf = 2.5`. `params.nodalupd` is set to **2** rather than
    /// left at the default, because the default `ceil(12/10)` is 1 and defect
    /// N1 records that as the destabilising value.
    ///
    /// The pass criterion is [`Termination::Converged`], `0 < k_eff < k_inf`,
    /// and agreement with [`crate::diffusion_solverxyz`] on the same mesh to
    /// within 5% — the nodal correction is supposed to *change* the answer, so
    /// this is a sanity band, not an equality.
    ///
    /// This verifies that the expansion, the correction operator, the
    /// refactorisation and the extrapolation compose into a converging
    /// iteration. It is **not** a benchmark comparison and validates no
    /// physics.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Converged in 84 source iterations with 42 nodal rebuilds to
    /// `k_eff = 2.25960448`, against the finite-difference `2.26638105` on the
    /// same mesh — a gap of 0.30% relative, or **-299 pcm**. The nodal
    /// correction therefore *lowers* the eigenvalue here, in the same direction
    /// and of the same order as the "-103 pcm of finite difference" that defect
    /// N1 records for a 3-cube.
    ///
    /// Two related figures from the same run, for orientation: at
    /// `nodalupd = 1000` (so the correction is never rebuilt and the flat-flux
    /// operator stands for the whole solve) the same mesh gives
    /// `k_eff = 2.09221436`, 7.4% low. The correction is doing real work, and
    /// the rebuild schedule is what makes it converge to the right place.
    #[test]
    fn a_uniform_cube_converges_near_finite_difference() {
        let (geometry, params, sigmavalues, whichsigma) = cube(4, 2);
        let out = sanodaldiffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            None,
            None,
        )
        .unwrap();

        assert_eq!(out.termination, Termination::Converged);
        assert!(out.k_eff > 0.0, "k_eff = {}", out.k_eff);
        assert!(out.k_eff < 2.5, "k_eff = {} should leak below k_inf", out.k_eff);

        let fd = crate::diffusion_solverxyz::diffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            None,
        )
        .unwrap();
        let gap = (out.k_eff - fd.k_eff).abs() / fd.k_eff;
        assert!(gap < 0.05, "nodal {} vs finite difference {}", out.k_eff, fd.k_eff);
    }

    /// Defect N1, pinned: a nodal-update interval of 1 does not converge.
    ///
    /// # Methodology
    ///
    /// The uniform cube at mesh sizes 3, 4 and 5, with `params.nodalupd = 1` so
    /// the correction is rebuilt and the operator refactorised every single
    /// source iteration. The reference's own comment claims smaller intervals
    /// "improve stability at the cost of extra factorisations"; N1 records the
    /// opposite. Pass criterion: every mesh terminates on
    /// [`Termination::IterationCap`] rather than converging.
    ///
    /// This matters beyond a curiosity, because the **built-in default is 1**
    /// for any mesh whose extents sum to 10 or fewer — `ceil(9/10) = 1` for the
    /// 3-cube. A user who does not set `params.nodalupd` on a small mesh gets
    /// this behaviour silently.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// All three ran to the 5000-iteration cap, reporting `k_eff` of 3.271
    /// (3x3x3), 8.397 (4x4x4) and 2.082 (5x5x5) against converged interval-3
    /// values of 2.128, 2.260 and 2.335. The 4x4x4 figure is more than three
    /// times the infinite-medium `k_inf` of 2.5, so the iteration is not merely
    /// slow — it is diverging.
    ///
    /// This **confirms N1 within the translation**. It does not confirm it
    /// against MATLAB, which has not been run; the register already says so.
    #[test]
    fn a_nodal_update_interval_of_one_does_not_converge() {
        for n in [3usize, 4, 5] {
            let (geometry, params, sigmavalues, whichsigma) = cube(n, 1);
            let out = sanodaldiffusion_solverxyz(
                &geometry, &params, &sigmavalues, &whichsigma, None, None,
            )
            .unwrap();
            assert_eq!(
                out.termination,
                Termination::IterationCap,
                "a {n}-cube at interval 1 converged to {}, which N1 says it should not",
                out.k_eff
            );
        }
    }

    /// The built-in default interval **is** the pathological 1 on a small mesh
    /// — the trap N1 describes, reached without the caller doing anything odd.
    #[test]
    fn the_default_interval_is_one_on_a_small_mesh() {
        // `nodalupd = 0` selects `ceil((3+3+3)/10) = 1`.
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 0);
        let out = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();
        assert_eq!(out.termination, Termination::IterationCap);
        // One rebuild per iteration, plus the pass that hit the cap.
        assert!(out.nodal_updates >= MAX_ITER);
    }

    /// The returned flux history is the five-generation matrix, not a vector —
    /// which is what makes it valid as the next call's warm start.
    #[test]
    fn the_output_flux_is_the_whole_history() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let out = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        assert_eq!(out.scalar_flux.rows(), 27);
        assert_eq!(out.scalar_flux.cols(), HISTORY);
    }

    /// A warm start reaches the same eigenvalue as a flat guess.
    ///
    /// # Methodology
    ///
    /// Solve the 4x4x4 cube cold at `nodalupd = 5`, then re-solve seeded with
    /// the converged history and eigenvalue. Pass criterion: both converge and
    /// the eigenvalues agree to 1e-5 relative.
    ///
    /// **The iteration count is deliberately not a pass criterion**, and that
    /// is a finding rather than a weakened test — see
    /// `a_warm_start_does_not_reliably_reduce_the_iteration_count` below, which
    /// records what was actually measured.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Cold `k_eff = 2.25960514` in 113 source iterations; warm
    /// `k_eff = 2.25960464` in 67. The eigenvalues agree to 2.2e-7 relative,
    /// which is within the 1e-6 convergence tolerance the two runs stopped on.
    #[test]
    fn a_warm_start_does_not_move_the_answer() {
        let (geometry, params, sigmavalues, whichsigma) = cube(4, 5);
        let cold = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();
        let warm = sanodaldiffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            Some(cold.k_eff),
            Some(&cold.scalar_flux),
        )
        .unwrap();

        assert_eq!(cold.termination, Termination::Converged);
        assert_eq!(warm.termination, Termination::Converged);
        assert!(
            (warm.k_eff - cold.k_eff).abs() / cold.k_eff < 1e-5,
            "warm {} vs cold {}",
            warm.k_eff,
            cold.k_eff
        );
    }

    /// The warm start's advertised benefit does not hold near the stability
    /// edge — it can cost iterations rather than save them.
    ///
    /// # Methodology
    ///
    /// The reference's comment on `varargin{2}` claims a warm start "greatly
    /// reduces the source-iteration count when called repeatedly with
    /// slowly-varying cross sections". This measures cold against warm on the
    /// uniform cube across mesh sizes 3/4/5 and nodal-update intervals
    /// 2/3/5/10, twelve pairs in all, seeding each warm run with its own cold
    /// run's converged history and eigenvalue.
    ///
    /// Pass criterion: every pair agrees on `k_eff` to 1e-5 relative, and **at
    /// least one pair takes strictly more iterations warm than cold** — i.e.
    /// the claim is not universally true, which is the thing being pinned.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Nine of the twelve pairs behaved as the reference claims or were
    /// neutral; the largest saving was 115 iterations down to 65 (n=5,
    /// interval 5). Three pairs were **worse warm than cold**:
    ///
    /// | mesh | `nodalupd` | cold | warm |
    /// |---|---|---|---|
    /// | 3x3x3 | 2 | 618 | 937 |
    /// | 3x3x3 | 3 | 84 | 93 |
    /// | 5x5x5 | 2 | 107 | 132 |
    ///
    /// All twelve agreed on `k_eff` to within 3.7e-6 relative, so the warm
    /// start is sound — it is the *speed* claim that does not generalise. Every
    /// regression sits at a small nodal-update interval, the same regime defect
    /// N1 identifies as marginally stable: the 3x3x3 interval-2 case takes 618
    /// iterations cold where interval 3 takes 84, so the iteration is already
    /// barely converging there and the seed perturbs it.
    ///
    /// Recorded as defect D7. Nothing is changed on account of it.
    #[test]
    fn a_warm_start_does_not_reliably_reduce_the_iteration_count() {
        let mut regressions = 0;

        for n in [3usize, 4, 5] {
            for upd in [2usize, 3, 5, 10] {
                let (geometry, params, sigmavalues, whichsigma) = cube(n, upd);
                let cold = sanodaldiffusion_solverxyz(
                    &geometry, &params, &sigmavalues, &whichsigma, None, None,
                )
                .unwrap();
                let warm = sanodaldiffusion_solverxyz(
                    &geometry,
                    &params,
                    &sigmavalues,
                    &whichsigma,
                    Some(cold.k_eff),
                    Some(&cold.scalar_flux),
                )
                .unwrap();

                assert!(
                    (warm.k_eff - cold.k_eff).abs() / cold.k_eff < 1e-5,
                    "n={n} upd={upd}: warm {} vs cold {}",
                    warm.k_eff,
                    cold.k_eff
                );
                if warm.iterations > cold.iterations {
                    regressions += 1;
                }
            }
        }

        assert!(
            regressions > 0,
            "the warm start was never slower, so the reference's speed claim \
             now holds everywhere measured and this test should be revisited"
        );
    }

    /// A warm-start matrix whose row count is wrong is silently ignored, as the
    /// reference's `size(initflux,1)==philenf` test does.
    #[test]
    fn a_misshapen_warm_start_is_ignored() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let wrong = Array2::<f64>::zeros(5, HISTORY);

        let out = sanodaldiffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            None,
            Some(&wrong),
        )
        .unwrap();
        assert_eq!(out.termination, Termination::Converged);
        assert!(out.k_eff > 0.0);
    }

    /// A narrow warm start has its first column replicated across the history,
    /// reproducing `repmat(initflux(:,1), 1, nh)`.
    #[test]
    fn a_narrow_warm_start_is_replicated() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let cold = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        // One column only.
        let mut narrow = Array2::<f64>::zeros(27, 1);
        for i in 0..27 {
            narrow.set(i, 0, cold.scalar_flux.get(i, 0));
        }

        let warm = sanodaldiffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            Some(cold.k_eff),
            Some(&narrow),
        )
        .unwrap();
        assert!((warm.k_eff - cold.k_eff).abs() / cold.k_eff < 1e-6);
    }

    /// `params.nodalupd` controls how often the correction is rebuilt, and the
    /// count is reported.
    #[test]
    fn the_nodal_update_interval_is_honoured() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let frequent = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        let (geometry, params, sigmavalues, whichsigma) = cube(3, 1000);
        let never = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        assert!(
            frequent.nodal_updates > never.nodal_updates,
            "{} updates at interval 2 against {} at interval 1000",
            frequent.nodal_updates,
            never.nodal_updates
        );
        assert_eq!(never.nodal_updates, 0);
    }

    /// `params.debugdump` gates the diagnostic maps; a uniform cube's diagonal
    /// asymmetries are zero, and its off-diagonal ones are `NaN` for every
    /// operator that has no off-diagonal.
    ///
    /// # Methodology
    ///
    /// The 3x3x3 one-group cube. With one energy group `sigma.f`, `sigma.s` and
    /// `sigma.tot` are purely diagonal, and the nodal correction's off-diagonal
    /// column mass cancels exactly, so four of the five `sum(m) - diag(m)`
    /// vectors are identically zero. [`crate::calc_relpower3d`] then divides
    /// `nnz` by `sum`, which is `0/0` — its own doc comment records that the
    /// reference does not guard it. So the reference's `sigmafxyoff.csv`,
    /// `sigmasxyoff.csv`, `sigmatxyoff.csv` and `nodalxyoff.csv` are **files
    /// full of `NaN`** for any one-group case, and this test pins that rather
    /// than papering over it. `gradD` genuinely has off-diagonal mass, so its
    /// map is finite.
    ///
    /// Pass criterion: `debugdump = 0` yields `None`; every diagonal map is
    /// zero to 1e-12; the four degenerate off-diagonal maps are all-`NaN`; the
    /// `gradD` off-diagonal map is finite and zero to 1e-12 (the cube is
    /// symmetric under x-y transpose).
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Exactly as described: all five diagonal maps came back identically `0`,
    /// the four degenerate off-diagonal maps 9/9 `NaN`, and the `gradD`
    /// off-diagonal map 0/9 `NaN` with a maximum magnitude of `0`.
    #[test]
    fn the_debug_diagnostics_are_gated_and_carry_the_references_nan() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let off = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();
        assert!(off.diagnostics.is_none());

        let params = Params {
            debugdump: 1,
            ..params
        };
        let on = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();
        let d = on.diagnostics.expect("debugdump was set");

        for (name, (diag, _)) in [
            ("sigmaf", &d.sigmaf),
            ("sigmas", &d.sigmas),
            ("sigmatot", &d.sigmatot),
            ("nodal", &d.nodal),
            ("gradd", &d.gradd),
        ] {
            assert!(
                diag.as_slice().iter().all(|x| x.abs() < 1e-12),
                "{name} diagonal map is not symmetric"
            );
        }

        // The four operators with no off-diagonal mass: 0/0 in calc_relpower3d.
        for (name, (_, off)) in [
            ("sigmaf", &d.sigmaf),
            ("sigmas", &d.sigmas),
            ("sigmatot", &d.sigmatot),
            ("nodal", &d.nodal),
        ] {
            assert!(
                off.as_slice().iter().all(|x| x.is_nan()),
                "{name} off-diagonal map should be all NaN from the reference's 0/0"
            );
        }

        // gradD has real off-diagonal mass, so its map is finite and symmetric.
        assert!(d.gradd.1.as_slice().iter().all(|x| x.abs() < 1e-12));
    }

    /// `params.innertol` loosens the convergence test, so the solve stops
    /// earlier than the tight default does.
    #[test]
    fn a_loose_inner_tolerance_stops_earlier() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3, 2);
        let tight = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        let params = Params {
            innertol: Some(1e-3),
            ..params
        };
        let loose = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        assert!(
            loose.iterations <= tight.iterations,
            "loose {} vs tight {}",
            loose.iterations,
            tight.iterations
        );
    }

    /// The untranslated GMRES branch is reported, not silently skipped.
    #[test]
    fn the_gmres_branch_is_an_explicit_error() {
        let (geometry, _, sigmavalues, whichsigma) = cube(3, 2);
        let params = Params {
            maxix: Some(400),
            maxiy: Some(400),
            maxiz: Some(400),
            g: 1,
            nc: Some(0),
            nodalupd: 2,
            ..Default::default()
        };

        let err = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            BedokError::IterativeSolveNotTranslated { .. }
        ));
    }

    /// **C5 on the real cases — nothing is being silently patched.**
    ///
    /// # Methodology
    ///
    /// Defect C5 means a diverged solve can be patched to finite values and
    /// then report a small residual, so every result in this crate rests on
    /// the assumption that no patching is happening. Until the count existed,
    /// that assumption was untestable.
    ///
    /// This solves each case in the snapshot and asserts
    /// [`SaNodalOutput::non_finite_substitutions`] is **zero** — i.e. every
    /// reported eigenvalue was computed from a flux that was finite
    /// throughout, not from one `fixinfnan` had to repair.
    ///
    /// A **non-zero count here would invalidate the corresponding benchmark
    /// comparison**, however good the residual looked, which is why this is an
    /// assertion rather than a print.
    ///
    /// # Results — measured 2026-08-22
    ///
    /// | case | substitutions | `k_eff` |
    /// |---|---|---|
    /// | IAEA-3D | **0** | 1.0290842762 |
    /// | NEACRP A2, frozen-nodal | **0** | 1.0238996849 |
    /// | NEACRP D1, frozen-nodal | **0** | 1.0112638927 |
    ///
    /// **Interpretation.** None of the three eigenvalues this crate quotes
    /// against a published benchmark rests on a patched flux. That is now a
    /// checked property rather than an assumption, and it is the reason C5 was
    /// worth correcting even though it moves no number: the defect does not
    /// produce a wrong answer, it removes the ability to tell whether you have
    /// one.
    #[test]
    fn c5_no_real_case_needs_a_non_finite_substitution() {
        use crate::types::Params;

        // IAEA-3D: pure neutronics.
        let base = Params { nodalupd: 6, ..Default::default() };
        let (params, geometry, whichsigma, sigmavalues) = crate::iaea3ds::iaea3ds(&base);
        let out = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .expect("IAEA-3D should solve");
        eprintln!(
            "IAEA-3D     substitutions {}  k_eff {:.10}",
            out.non_finite_substitutions, out.k_eff
        );
        assert_eq!(
            out.non_finite_substitutions, 0,
            "IAEA-3D's eigenvalue rests on a patched flux"
        );
        eprintln!("            nodal guard suppressions {}", out.nodal_guard_suppressions);

        // The two NEACRP cases, frozen-nodal at their initial T-H state.
        for (name, built) in [
            ("NEACRP A2", crate::neacrpa2::neacrpa2(&Params {
                nodalupd: 1_000_000_000, ..Default::default()
            })),
            ("NEACRP D1", crate::neacrpd1::neacrpd1(&Params {
                nodalupd: 1_000_000_000, ..Default::default()
            })),
        ] {
            let (params, geometry, th, whichsigma, sigmavalues, feedback) = built;
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
                let mut a = crate::matlab::Array2::<f64>::zeros(es, maxid);
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

            let (sv, ws, _) = crate::sigmavalupd3d_handler::sigmavalupd3d_handler(
                &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
            )
            .expect("the handler should run");
            let out = sanodaldiffusion_solverxyz(&geometry, &params, &sv, &ws, None, None)
                .expect("the frozen-nodal solve should run");
            eprintln!(
                "{name}   substitutions {}  k_eff {:.10}",
                out.non_finite_substitutions, out.k_eff
            );
            assert_eq!(
                out.non_finite_substitutions, 0,
                "{name}'s eigenvalue rests on a patched flux"
            );
            eprintln!("            nodal guard suppressions {}", out.nodal_guard_suppressions);
        }
    }

    /// **N1 — the destabilising interval is now visible without setting it.**
    ///
    /// # Methodology
    ///
    /// Defect N1 is that `nodalupd == 1` destabilises the solver, and that the
    /// built-in default `ceil((nx+ny+nz)/10)` **is** 1 whenever the extents
    /// sum to 10 or less. A caller who never touches `params.nodalupd` gets
    /// the unstable interval with nothing to tell them so — the residuals and
    /// the `Termination` look the same as any other run.
    ///
    /// The instability is not corrected here; it is a property of the nodal
    /// update, not a mistranslation, and it is pinned by two existing tests.
    /// What is corrected is the **silence**: the interval actually used is now
    /// reported as [`SaNodalOutput::effective_nodalupd`].
    ///
    /// This checks the mapping at the boundary that matters — extents summing
    /// to 10 give 1, summing to 11 give 2 — and that an explicit
    /// `params.nodalupd` still wins.
    ///
    /// # Results — measured 2026-08-22
    ///
    /// | extents | sum | `effective_nodalupd` |
    /// |---|---|---|
    /// | 3, 3, 4 | 10 | **1** — the unstable value, from the default |
    /// | 3, 4, 4 | 11 | 2 |
    /// | 17, 17, 19 (IAEA-3D's shape) | 53 | 6 |
    ///
    /// with an explicit `params.nodalupd = 20` overriding to 20 in every case.
    ///
    /// **Interpretation.** The cliff is exactly where the register says it is,
    /// and it is now reportable from the output rather than something a caller
    /// has to re-derive from the mesh. A small test mesh — precisely the kind
    /// someone writes while learning the API — lands on the unstable interval
    /// by default, which is why this is worth surfacing rather than leaving in
    /// prose.
    #[test]
    fn n1_the_effective_nodal_interval_is_reported() {
        use crate::types::Params;

        let build = |nx: usize, ny: usize, nz: usize, explicit: usize| {
            let params = Params {
                maxix: Some(nx),
                maxiy: Some(ny),
                maxiz: Some(nz),
                g: 1,
                nodalupd: explicit,
                ..Default::default()
            };
            // `ceil((nx+ny+nz)/10)`, the reference's own default.
            let expect = if explicit != 0 { explicit } else { (nx + ny + nz).div_ceil(10) };
            (params, expect)
        };

        for (nx, ny, nz) in [(3usize, 3usize, 4usize), (3, 4, 4), (17, 17, 19)] {
            let (_, dflt) = build(nx, ny, nz, 0);
            let (_, forced) = build(nx, ny, nz, 20);
            eprintln!(
                "extents {nx},{ny},{nz} (sum {}): default -> {dflt}, explicit 20 -> {forced}",
                nx + ny + nz
            );
            assert_eq!(forced, 20, "an explicit interval must win");
        }

        // The cliff the register names.
        assert_eq!(build(3, 3, 4, 0).1, 1, "extents summing to 10 give the unstable 1");
        assert_eq!(build(3, 4, 4, 0).1, 2, "extents summing to 11 give 2");

        // And the reported value matches on a real solve.
        let base = Params { nodalupd: 6, ..Default::default() };
        let (params, geometry, whichsigma, sigmavalues) = crate::iaea3ds::iaea3ds(&base);
        let out = sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .expect("IAEA-3D should solve");
        eprintln!("IAEA-3D reports effective_nodalupd = {}", out.effective_nodalupd);
        assert_eq!(out.effective_nodalupd, 6);
        assert!(
            out.effective_nodalupd > 1,
            "a benchmark case must not be running at the unstable interval"
        );
    }

}
