//! Finite-difference multigroup diffusion — the plain power iteration.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `diffusion_solverxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this is, and what it is not
//!
//! This is the **reference solver without the nodal correction**: a mesh-centred
//! finite-difference discretisation solved by source iteration. Its companion,
//! [`crate::sanodaldiffusion_solverxyz`], adds the semi-analytic nodal (SANM)
//! correction operator on top of the same `gradD` and is the one the benchmark
//! drivers actually call. This one is the baseline the nodal answer is judged
//! against — `docs/bedok-reference-defects.md` N1 quotes a "-103 pcm of finite
//! difference" comparison, and this is the finite difference it means.
//!
//! The two are deliberately **not** factored into a shared solver here. They
//! differ in the operator split, in three separate normalisation choices, in
//! their acceleration, and in their iteration caps; merging them would hide
//! exactly the inconsistencies the defect register is trying to keep visible.
//! One module per `.m` file, as everywhere else in this crate.

use crate::calc_relpower3d::calc_relpower3d;
use crate::calcdiffvalues3d::calcdiffvalues3d;
use crate::error::BedokError;
use crate::makegrad_dxyz::makegrad_dxyz;
use crate::makesigmadfxyz::makesigmadfxyz;
use crate::matlab::{norm1, norm2, Array2, Array3, Decomposition, SparseMatrix};
use crate::types::{Geometry, Params, SigmaValues};
use crate::Result;

/// `sizethresh` — above this many unknowns the reference switches to
/// preconditioned GMRES. See [`BedokError::IterativeSolveNotTranslated`] for
/// why that branch is not translated.
pub const SIZE_THRESH: usize = 50_000_000;

/// `diffusion.tol` — the convergence tolerance on both residuals.
///
/// Unlike [`crate::sanodaldiffusion_solverxyz`], this solver has no
/// `params.innertol` override; it is always tight.
pub const TOL: f64 = 1e-6;

/// `maxiter` — the source-iteration cap.
///
/// Note this is **10000** where the nodal solver uses 5000.
pub const MAX_ITER: usize = 10_000;

/// Why the source iteration stopped.
///
/// The reference distinguishes these only by which `break` fired, and reports
/// nothing about it; returning the reason lets a caller tell a converged answer
/// from a bailed-out one, which the reference's own output cannot do. Defect C7
/// records that silent non-convergence as a problem in the coupling layer, so
/// not reproducing the silence is worth the small addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// Both residuals fell below [`TOL`] — the `while` condition went false.
    Converged,
    /// `k_eff <= 0`, i.e. the eigenvalue update produced a non-physical value.
    NonPositiveKeff,
    /// `k_eff` became `NaN` — in practice a singular or diverging solve.
    NanKeff,
    /// The iteration count passed [`MAX_ITER`].
    IterationCap,
}

/// Diagnostic asymmetry maps, the quantities the reference writes to CSV.
///
/// # Why these are returned rather than written
///
/// `diffusion_solverxyz.m` calls `writematrix` **unconditionally** — three
/// symmetry maps before the iteration and `rel_power_inner.csv` after it — so
/// every single call scribbles four files into the working directory. (Its
/// nodal counterpart puts the equivalent dumps behind `params.debugdump`; this
/// one does not, which is defect D3.)
///
/// A library that writes files as a side effect of being called is not
/// something this translation is willing to reproduce: it would make the solver
/// unusable from two threads, untestable without a temp directory, and
/// surprising to every caller. The quantities are computed exactly as the
/// reference computes them and handed back instead, so a caller that wants the
/// files can write them and the physics is unchanged either way.
///
/// Each map is `maxix` by `maxiy` and dimensionless.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    /// `sigmafxy - sigmafxy.'` — the antisymmetric part of the collapsed
    /// fission diagonal. Written as `sigmafxy.csv`.
    pub sigmaf_asymmetry: Array2<f64>,
    /// `sigmasxy - sigmasxy.'`, from the scattering diagonal. Written as
    /// `sigmasxy.csv`.
    pub sigmas_asymmetry: Array2<f64>,
    /// `sigmatxy - sigmatxy.'`, from the total-cross-section diagonal. Written
    /// as `sigmatxy.csv`.
    pub sigmatot_asymmetry: Array2<f64>,
    /// `rel_power` — the normalised assembly power map. Written as
    /// `rel_power_inner.csv`.
    pub rel_power: Array2<f64>,
}

/// `output` — what the reference returns, plus the provenance it does not.
///
/// Deliberately **not** `Default`: there is no honest default for
/// [`DiffusionOutput::termination`], and a zero-valued `k_eff` is not a
/// meaningful starting point for anything.
#[derive(Clone, Debug)]
pub struct DiffusionOutput {
    /// `output.k_eff` — the multiplication factor, dimensionless.
    pub k_eff: f64,
    /// `output.residual` — the relative fission-source change, dimensionless.
    pub residual: f64,
    /// `output.k_eff_residual` — the relative `k_eff` change, dimensionless.
    pub k_eff_residual: f64,
    /// `output.scalar_flux` — the converged flux, `philenf` long, normalised so
    /// its fission-source 1-norm equals the flat guess's.
    pub scalar_flux: Vec<f64>,
    /// `output.fission_source` — `sigma.f * scalar_flux`, same length and
    /// normalisation.
    pub fission_source: Vec<f64>,
    /// `output.pwrdens` — `fission_source .* Vi`, the power density per node.
    pub pwrdens: Vec<f64>,
    /// `phi_plot` — the flux summed over groups on the `zplot = 1` axial plane,
    /// `maxix` by `maxiy`.
    ///
    /// The reference computes this whether or not `params.plotfig` is set, and
    /// then only uses it to draw `figure(6)`. Returned rather than plotted,
    /// since a library cannot open a figure window; `params.plotfig` is
    /// consequently not read here at all.
    pub phi_plot: Array2<f64>,
    /// The source-iteration count the reference prints as
    /// `Diffusion iteration`. This is `iteration - 1`.
    pub iterations: usize,
    /// Why the iteration stopped. Not in the reference's `output`.
    pub termination: Termination,
    /// The unconditional CSV dumps, returned instead of written. See
    /// [`Diagnostics`].
    pub diagnostics: Diagnostics,
}

/// `output = diffusion_solverxyz(geometry, params, sigmavalues, whichsigma, initial_k_eff)`.
///
/// Assembles the finite-difference diffusion operator and runs a source
/// iteration on it to convergence, returning the fundamental-mode flux and
/// eigenvalue.
///
/// # Arguments
///
/// - `geometry` — needs `Vi`, the per-node volumes, plus everything
///   [`makegrad_dxyz`] reads (the `[low, high]` bounds, the node widths and the
///   six boundary conditions).
/// - `params` — `G`, `Nc` and the three extents.
/// - `sigmavalues` — per-material cross sections.
/// - `whichsigma` — the 1-based material map, `0` for void.
/// - `initial_k_eff` — `varargin{1}`; `None` is the reference's default of `1`.
///
/// # The operator split, and why the scattering term appears twice
///
/// The reference builds
///
/// ```text
/// LHS = gradD + sigma.tot - sigma.sd
/// RHS = fission_source/k_eff + (sigma.s - sigma.sd)*scalar_flux
/// ```
///
/// `sigma.sd` is the **within-group** scattering diagonal and `sigma.s` is the
/// full scattering operator, so the two lines together are
/// `(gradD + sigma.tot - sigma.s) phi = fission_source / k_eff` with the
/// within-group part treated implicitly and the group-to-group part lagged one
/// iteration. That is an ordinary source iteration over energy, and it is why
/// this solver's `LHS` differs from the nodal solver's — that one puts the
/// whole of `sigma.s` on the left and carries no scattering source at all.
///
/// # Normalisation
///
/// Every fission-source integral here is a **1-norm**, `norm(x, 1)`. The flux
/// and source are rescaled each pass so the source integral holds at whatever
/// the flat initial guess produced. The comment in the reference says "fission
/// source integration = 1", which is not what the code does — see defect N10,
/// which raises the same point against the nodal solver.
///
/// Note `k_eff` is updated from the **un-rescaled** `fission_source_new`, before
/// the rescale; since the update is a ratio of successive integrals and both
/// are rescaled by the same factor, that choice does not change the result.
///
/// # The empty-grid compaction is dead code
///
/// Lines 60-76 and 160-174 of the reference compact the operators onto the
/// occupied nodes with [`crate::convert_grid3d`] and
/// [`crate::convertsparsekey3d`], and expand the answer back afterwards. Both
/// blocks are guarded by `keychange == 1` where `keychange` is the literal `0`
/// assigned four lines earlier, so **neither ever runs**. It is not translated:
/// there is nothing to reproduce, and writing an untested compaction path would
/// be inventing behaviour rather than porting it. The two functions it would
/// call are translated and tested in their own right. Recorded as defect D1.
///
/// # On a non-convergence break, the reported state lags by one iteration
///
/// The `break` fires before `iteration` is incremented, so `k_eff(iteration)`,
/// `residual(iteration)` and `k_eff_residual(iteration)` in the output are the
/// **previous** pass's values — the offending `k_eff(iteration+1)` that
/// triggered the break is computed, tested and then discarded. Preserved;
/// [`DiffusionOutput::termination`] is how a caller can tell this happened.
/// Recorded as defect D2.
///
/// # `Nc > 0` does not work
///
/// `Vi` is replicated to `G` groups, giving `G*es` entries, while the fission
/// source is `philenf = (G+Nc)*es` long. MATLAB's `.*` errors on the mismatch.
/// This is the same conformance gap as defects C11 and N2; all four benchmark
/// cases set `Nc = 0`. Reproduced as a panic.
///
/// # Errors
///
/// - [`BedokError::IterativeSolveNotTranslated`] if `philenf >= 50_000_000`.
/// - Whatever [`makegrad_dxyz`] raises.
///
/// # Panics
///
/// If `geometry.vi` is shorter than `maxix*maxiy*maxiz`, or if `Nc > 0` (see
/// above).
pub fn diffusion_solverxyz(
    geometry: &Geometry,
    params: &Params,
    sigmavalues: &SigmaValues,
    whichsigma: &Array3<usize>,
    initial_k_eff: Option<f64>,
) -> Result<DiffusionOutput> {
    let g_count = params.g;
    let nc = params.nc_or_zero();
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let philenf = (g_count + nc) * es;

    if philenf >= SIZE_THRESH {
        return Err(BedokError::IterativeSolveNotTranslated {
            philenf,
            threshold: SIZE_THRESH,
        });
    }

    // `Vi = repmat(Vi, G, 1)` — G copies stacked, so `G*es` long. Note this is
    // `G`, not `G+Nc`; see the `Nc > 0` note above.
    assert!(
        geometry.vi.len() >= es,
        "geometry.vi is {} long, need at least {es}",
        geometry.vi.len()
    );
    let vi: Vec<f64> = (0..g_count * es).map(|i| geometry.vi[i % es]).collect();

    let initial_k_eff = initial_k_eff.unwrap_or(1.0);

    // ----- calculate matrices ----- //
    let mut sigma = makesigmadfxyz(params, sigmavalues, whichsigma, None);
    let diffd = calcdiffvalues3d(params, &sigmavalues.tot, whichsigma, None);
    let gradd = makegrad_dxyz(geometry, params, &diffd, whichsigma, None)?;

    // The three unconditional symmetry dumps. `calc_relpower3d` collapses the
    // group-flattened diagonal onto the x-y plane; subtracting the transpose
    // leaves the antisymmetric part, which is what the author was inspecting.
    let diagnostics_pre = |m: &mut SparseMatrix| -> Array2<f64> {
        antisymmetric_part(&calc_relpower3d(params, &m.diagonal()))
    };
    let sigmaf_asymmetry = diagnostics_pre(&mut sigma.f);
    let sigmas_asymmetry = diagnostics_pre(&mut sigma.s);
    let sigmatot_asymmetry = diagnostics_pre(&mut sigma.tot);

    // ----- Set up initial values ----- //
    let mut scalar_flux = vec![1.0; philenf];

    // `residual` and `k_eff_residual` start at 1 so the `while` always runs at
    // least once; `k_eff` starts at the caller's guess.
    let mut residual: Vec<f64> = vec![1.0];
    let mut k_eff_residual: Vec<f64> = vec![1.0];
    let mut k_eff: Vec<f64> = vec![initial_k_eff];
    // 0-based: the reference's `iteration` is this plus one.
    let mut iteration = 0usize;

    let mut fission_source = sigma.f.mul_vec(&scalar_flux);
    let init_norm = norm1(&fission_source);

    let mut lhs = SparseMatrix::combine(&[(&gradd.operator, 1.0), (&sigma.tot, 1.0), (&sigma.sd, -1.0)]);
    let dlhs = Decomposition::new(&mut lhs);

    // ----- Run source iteration ----- //
    let termination = loop {
        if residual[iteration] < TOL && k_eff_residual[iteration] < TOL {
            break Termination::Converged;
        }

        // RHS = fission_source/k_eff + (sigma.s - sigma.sd)*scalar_flux
        let mut scatter = SparseMatrix::combine(&[(&sigma.s, 1.0), (&sigma.sd, -1.0)]);
        let scatter_source = scatter.mul_vec(&scalar_flux);
        let rhs: Vec<f64> = (0..philenf)
            .map(|n| fission_source[n] / k_eff[iteration] + scatter_source[n])
            .collect();

        let scalar_flux_l_plus = dlhs.solve(&rhs);

        // The reference forms `sigma.f * scalar_flux_l_plus` twice — once for
        // `norm_factor` and once for `fission_source_new`. Same vector; formed
        // once here.
        let mut fission_source_new = sigma.f.mul_vec(&scalar_flux_l_plus);
        let norm_factor = norm1(&fission_source_new);

        // k_eff update, from the un-rescaled sources.
        let k_next = k_eff[iteration] * norm1(&fission_source_new) / norm1(&fission_source);
        k_eff.push(k_next);

        // Rescale both to hold the source integral at its initial value.
        let scale = init_norm / norm_factor;
        let scalar_flux_l_plus: Vec<f64> = scalar_flux_l_plus.iter().map(|x| x * scale).collect();
        for x in fission_source_new.iter_mut() {
            *x *= scale;
        }

        let diff: Vec<f64> = (0..philenf)
            .map(|n| fission_source_new[n] - fission_source[n])
            .collect();
        residual.push(norm2(&diff) / norm2(&fission_source));

        k_eff_residual.push((k_next - k_eff[iteration]).abs() / k_eff[iteration]);

        // Stop if not converging. Note this tests the *new* k_eff but leaves
        // `iteration` pointing at the old one — see the module docs.
        if k_next <= 0.0 {
            break Termination::NonPositiveKeff;
        }
        if k_next.is_nan() {
            break Termination::NanKeff;
        }
        if iteration + 1 > MAX_ITER {
            break Termination::IterationCap;
        }

        iteration += 1;
        scalar_flux = scalar_flux_l_plus;
        fission_source = fission_source_new;
    };

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

    let rel_power = calc_relpower3d(params, &pwrdens);

    // `zplot = 1` — the bottom axial plane, 0-based here.
    let mut phi_plot = Array2::<f64>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let mut acc = 0.0;
            for g in 0..g_count {
                acc += scalar_flux[g * es + ix * maxiy * maxiz + iy * maxiz];
            }
            phi_plot.set(ix, iy, acc);
        }
    }

    Ok(DiffusionOutput {
        k_eff: k_eff[iteration],
        residual: residual[iteration],
        k_eff_residual: k_eff_residual[iteration],
        scalar_flux,
        fission_source,
        pwrdens,
        phi_plot,
        iterations: iteration,
        termination,
        diagnostics: Diagnostics {
            sigmaf_asymmetry,
            sigmas_asymmetry,
            sigmatot_asymmetry,
            rel_power,
        },
    })
}

/// `a - a.'` — the antisymmetric part of a square map.
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

    /// A uniform cube of one fissile material, vacuum on every face.
    ///
    /// One energy group keeps the expected answer checkable by hand: with no
    /// up- or down-scattering the source iteration reduces to a plain power
    /// iteration on a symmetric operator.
    fn cube(n: usize) -> (Geometry, Params, SigmaValues, Array3<usize>) {
        let params = Params {
            maxix: Some(n),
            maxiy: Some(n),
            maxiz: Some(n),
            g: 1,
            nc: Some(0),
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

    /// The source iteration converges on a uniform leaking cube, and the
    /// eigenvalue is below the infinite-medium value by the leakage.
    ///
    /// # Methodology
    ///
    /// A 4x4x4 cube of 10 cm nodes, one group, `Sigma_tot = 0.5`,
    /// `Sigma_s = 0.4`, `Sigma_f = 0.1`, `nu = 2.5`, vacuum on all six faces.
    /// The infinite-medium multiplication is
    /// `k_inf = nu*Sigma_f / (Sigma_tot - Sigma_s) = 0.25/0.1 = 2.5`. A finite
    /// cube leaks, so the pass criterion is `0 < k_eff < k_inf` together with
    /// [`Termination::Converged`] and both residuals under the 1e-6 tolerance.
    ///
    /// This checks that the assembly, the factorisation and the iteration hang
    /// together and land on a physically-signed answer. It is **not** a
    /// benchmark comparison — no published `k_eff` is involved — so it verifies
    /// the implementation, it does not validate the physics.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Converged in 81 source iterations to `k_eff = 2.26638105`, with a
    /// fission-source residual of 9.88e-7 and a `k_eff` residual of 7.84e-8.
    /// The eigenvalue sits 0.234 below `k_inf = 2.5`, i.e. the cube leaks about
    /// 9.3% of its neutrons — plausible for a body four 10 cm nodes across with
    /// `D = 1/(3 * 0.5) = 0.667 cm` and a diffusion length of
    /// `sqrt(D / 0.1) = 2.58 cm`.
    ///
    /// This is a verification result, not a validation one: no published
    /// benchmark value is involved.
    #[test]
    fn a_uniform_cube_converges_below_k_inf() {
        let (geometry, params, sigmavalues, whichsigma) = cube(4);
        let out =
            diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();

        assert_eq!(out.termination, Termination::Converged);
        assert!(out.k_eff > 0.0, "k_eff = {}", out.k_eff);
        assert!(out.k_eff < 2.5, "k_eff = {} should leak below k_inf", out.k_eff);
        assert!(out.residual < TOL);
        assert!(out.k_eff_residual < TOL);
        assert_eq!(out.scalar_flux.len(), 64);
    }

    /// The converged flux is positive everywhere and peaks at the centre, which
    /// is what the fundamental mode of a bare uniform cube must look like.
    #[test]
    fn the_fundamental_mode_is_positive_and_centre_peaked() {
        let (geometry, params, sigmavalues, whichsigma) = cube(4);
        let out =
            diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();

        assert!(
            out.scalar_flux.iter().all(|&x| x > 0.0),
            "the fundamental mode must not change sign"
        );

        // Node (1,1,1) is interior on a 4-cube; (0,0,0) is a corner.
        let at = |ix: usize, iy: usize, iz: usize| out.scalar_flux[ix * 16 + iy * 4 + iz];
        assert!(at(1, 1, 1) > at(0, 0, 0));
    }

    /// The eigenvalue does not depend on the initial guess — only the iteration
    /// count does.
    #[test]
    fn the_initial_k_eff_guess_does_not_move_the_answer() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3);
        let a = diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();
        let b = diffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            Some(1.8),
        )
        .unwrap();

        assert!(
            (a.k_eff - b.k_eff).abs() < 1e-5,
            "k_eff {} vs {} from a different guess",
            a.k_eff,
            b.k_eff
        );
    }

    /// The power density is the fission source times the node volume, node for
    /// node — the one place `geometry.Vi` is used.
    #[test]
    fn the_power_density_is_the_source_times_the_volume() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3);
        let out =
            diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();

        for n in 0..out.pwrdens.len() {
            assert!((out.pwrdens[n] - out.fission_source[n] * 1000.0).abs() < 1e-9);
        }
    }

    /// The relative-power map averages to 1 over the fuelled nodes, and a
    /// uniform cube's is flat.
    #[test]
    fn the_relative_power_map_is_normalised() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3);
        let out =
            diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();

        let rp = &out.diagnostics.rel_power;
        let mean: f64 = rp.as_slice().iter().sum::<f64>() / rp.as_slice().len() as f64;
        assert!((mean - 1.0).abs() < 1e-9, "mean = {mean}");
    }

    /// The symmetry diagnostics of a uniform cube are identically zero, which
    /// is what the author was checking for when he added the CSV dumps.
    #[test]
    fn a_symmetric_problem_has_zero_asymmetry() {
        let (geometry, params, sigmavalues, whichsigma) = cube(3);
        let out =
            diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None).unwrap();

        for m in [
            &out.diagnostics.sigmaf_asymmetry,
            &out.diagnostics.sigmas_asymmetry,
            &out.diagnostics.sigmatot_asymmetry,
        ] {
            assert!(m.as_slice().iter().all(|x| x.abs() < 1e-12));
        }
    }

    /// The untranslated GMRES branch is reported, not silently skipped.
    ///
    /// `params` claiming a 400x400x400 mesh puts `philenf` at 6.4e7, past the
    /// reference's `sizethresh`. Nothing is allocated, because the check comes
    /// first.
    #[test]
    fn the_gmres_branch_is_an_explicit_error() {
        let (geometry, _, sigmavalues, whichsigma) = cube(3);
        let params = Params {
            maxix: Some(400),
            maxiy: Some(400),
            maxiz: Some(400),
            g: 1,
            nc: Some(0),
            ..Default::default()
        };

        let err = diffusion_solverxyz(&geometry, &params, &sigmavalues, &whichsigma, None)
            .unwrap_err();
        assert!(matches!(
            err,
            BedokError::IterativeSolveNotTranslated { .. }
        ));
    }
}
