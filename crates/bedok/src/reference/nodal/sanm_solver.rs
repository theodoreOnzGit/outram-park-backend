//! The semi-analytic nodal `k`-eigenvalue solver.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `sanodaldiffusion_solverxyz.m` (function
//! `sanodaldiffusion_solverxyz`).

use super::cross_sections::{self, CrossSectionOperators, MaterialCrossSections};
use super::fission_source;
use super::flux_history::FluxHistory;
use super::geometry::{NodalGeometry, NodalParams};
use super::gradient_diffusion;
use super::nodal_coefficients;
use super::nodal_correction;
use super::sparse::SparseMatrix;

/// Hard iteration ceiling — MATLAB `maxiter=5000`.
pub const MAX_ITERATIONS: usize = 5000;

/// Why the source iteration stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// Both the fission-source residual and the `k_eff` residual fell below the
    /// tolerance.
    Converged,
    /// `k_eff` went non-positive or `NaN`, or the iteration ceiling was
    /// reached. The MATLAB prints "Source interation stopped, not converging",
    /// dumps the flux to `scalar_fluxerr.csv`, and returns the values it has —
    /// it does **not** raise an error, and neither does this port.
    Stopped,
}

/// The result of a source iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffusionSolution {
    /// Converged multiplication factor \[dimensionless\]. MATLAB
    /// `output.k_eff`.
    pub k_eff: f64,
    /// Final relative fission-source residual \[dimensionless\]. MATLAB
    /// `output.residual`.
    pub fission_source_residual: f64,
    /// Final relative `k_eff` residual \[dimensionless\]. MATLAB
    /// `output.k_eff_residual`.
    pub k_eff_residual: f64,
    /// The flux history, renormalised so the fission-source integral matches
    /// its initial value. MATLAB `output.scalar_flux`.
    pub scalar_flux: FluxHistory,
    /// Fission source per unit volume \[neutrons cm⁻³ s⁻¹\]. MATLAB
    /// `output.fission_source`.
    pub fission_source: Vec<f64>,
    /// Node power density, `fission_source * node volume`
    /// \[neutrons s⁻¹ per node\]. MATLAB `output.pwrdens`.
    pub power_density: Vec<f64>,
    /// Source iterations performed. MATLAB's `iteration-1` in the printout.
    pub iterations: usize,
    /// Why the loop ended.
    pub termination: Termination,
}

/// Solves the `k`-eigenvalue problem with the semi-analytic nodal method —
/// `sanodaldiffusion_solverxyz.m`.
///
/// # What it does
///
/// A power (source) iteration on
///
/// ```text
/// (gradD + nodal + sigma_tot - sigma_s) phi = (1/k) F phi
/// ```
///
/// solved each pass with a **direct** sparse LU factorisation, refactorising
/// only when the nodal correction is rebuilt. `k_eff` is updated by the ratio
/// of successive fission-source 1-norms, and every
/// [`NodalParams::fission_extrapolation_interval`] iterations the flux and
/// fission source are extrapolated (see [`super::fission_source`]).
///
/// Convergence requires **both** the relative fission-source residual and the
/// relative `k_eff` residual to fall below
/// [`NodalParams::inner_tolerance`].
///
/// # Stability caveat
///
/// A [`NodalParams::nodal_update_interval`] of 1 — the MATLAB default whenever
/// `nx+ny+nz <= 10` — makes the iteration unstable in this port. Read that
/// field's documentation before using a small mesh.
///
/// # Arguments
///
/// - `geometry` — node dimensions, boundary conditions and discontinuity
///   factors. Its `nodal_coefficients` field is (re)computed here, exactly as
///   the MATLAB assigns `geometry.nodalcoeffs` inside the solver.
/// - `values` — per-material multigroup cross sections \[cm⁻¹\].
/// - `which_sigma` — 1-based material index per spatial node, `0` for none.
/// - `initial_k_eff` — starting eigenvalue \[dimensionless\]; the MATLAB
///   default is 1.
/// - `initial_flux` — optional warm start. If its depth matches the history
///   depth the columns are taken as-is; otherwise the current column is
///   broadcast, mirroring the reference's two-branch warm start.
///
/// # Normalisation
///
/// On exit the flux and fission source are scaled so that the **sum** (not the
/// 1-norm) of the fission source equals its value on the first iterate. The
/// MATLAB comment reads "CURRENT NORMALIZATION: fission source intergration =
/// 1", which is not what the code does — it preserves the initial integral,
/// whatever that was. Recorded, not changed.
///
/// # Not ported
///
/// - The `params.debugdump` CSV writes and the `params.plotfig` surface plot.
///   The MATLAB itself calls these "pure side effects [that] do not affect the
///   solver's return values".
/// - The `philenf >= sizethresh` branch, which switches to preconditioned
///   GMRES. `sizethresh` is 5×10⁷, far above any state vector the benchmarks
///   produce, so it is dead in every case in the snapshot — and substituting an
///   iterative solver would be a stage-2 change (`docs/bedok-port-scoping.md`
///   §5), not a translation.
/// - The commented-out Wielandt shift. `weilandtfactor` is set and never used;
///   the author's note says it "does not seem to work".
///
/// # Panics
///
/// If the LHS cannot be factorised, or if `Nc > 0` makes the operator shapes
/// disagree (see
/// [`NodalParams::n_precursor_groups`](super::geometry::NodalParams::n_precursor_groups)).
#[must_use]
// `iteration % interval == 0` is MATLAB's `mod(iteration,nodalupd)==0`,
// kept in that form so the two read the same.
#[allow(clippy::manual_is_multiple_of)]
pub fn solve(
    params: &NodalParams,
    geometry: &NodalGeometry,
    values: &MaterialCrossSections,
    which_sigma: &[usize],
    initial_k_eff: f64,
    initial_flux: Option<&FluxHistory>,
) -> DiffusionSolution {
    let grid = params.grid;
    let philenf = params.philenf();

    // ----- calculate matrices -----
    let sigma = cross_sections::assemble_operators(params, values, which_sigma);
    let diffusion = cross_sections::diffusion_coefficients(grid, &values.total, which_sigma, 1.0);
    let grad = gradient_diffusion::assemble(params, geometry, &diffusion, which_sigma);

    let mut geometry = geometry.clone();
    geometry.nodal_coefficients =
        nodal_coefficients::assemble(params, &geometry, &sigma.total, &sigma.scatter, &diffusion);

    let flat_flux = vec![1.0; philenf];
    let mut correction = nodal_correction::assemble(
        params,
        &geometry,
        &flat_flux,
        &sigma,
        &diffusion,
        &grad.face_terms,
        &super::geometry::FaceTerms::zeros(params.philen()),
        1.0,
    );

    // ----- set up initial values -----
    let mut scalar_flux = match initial_flux {
        Some(h) if h.len() == philenf => {
            if h.depth() >= FluxHistory::DEFAULT_DEPTH {
                FluxHistory::from_columns(
                    (0..FluxHistory::DEFAULT_DEPTH)
                        .map(|j| h.column(j).to_vec())
                        .collect(),
                )
            } else {
                FluxHistory::broadcast(h.current(), FluxHistory::DEFAULT_DEPTH)
            }
        }
        _ => FluxHistory::filled(philenf, FluxHistory::DEFAULT_DEPTH, 1.0),
    };

    let mut residual = vec![1.0];
    let mut k_eff_residual = vec![1.0];
    let mut k_eff = vec![initial_k_eff];
    let mut iter = 0usize;

    let mut fission_source_vec = sigma.fission.mul_vec(scalar_flux.current());
    let init_norm: f64 = fission_source_vec.iter().sum();

    let mut lhs = build_lhs(&grad.operator, &correction.operator, &sigma);
    let mut lu = lhs.lu().expect("the SANM LHS is nonsingular");

    let mut fission_source_new = fission_source_vec.clone();
    let mut termination = Termination::Converged;

    // ----- run source iteration -----
    while residual[iter] >= params.inner_tolerance || k_eff_residual[iter] >= params.inner_tolerance
    {
        let iteration = iter + 1; // the MATLAB's 1-based counter

        if params.nodal_update_interval != 0 && iteration % params.nodal_update_interval == 0 {
            correction = nodal_correction::assemble(
                params,
                &geometry,
                scalar_flux.current(),
                &sigma,
                &diffusion,
                &grad.face_terms,
                &correction.face_terms,
                k_eff[iter],
            );
            lhs = build_lhs(&grad.operator, &correction.operator, &sigma);
            lu = lhs.lu().expect("the SANM LHS is nonsingular");
        }

        let rhs: Vec<f64> = fission_source_vec.iter().map(|v| v / k_eff[iter]).collect();
        let mut flux_next = lu.solve(&rhs);
        fix_inf_nan(&mut flux_next);

        fission_source_new = sigma.fission.mul_vec(&flux_next);
        scalar_flux.push(flux_next);

        if params.fission_extrapolation_interval != 0
            && iteration % params.fission_extrapolation_interval == 0
        {
            let (fs, _) = fission_source::extrapolate(&sigma.fission, &mut scalar_flux);
            fission_source_new = fs;
        }

        let next_k = k_eff[iter] * norm1(&fission_source_new) / norm1(&fission_source_vec);
        k_eff.push(next_k);
        residual.push(
            norm2_difference(&fission_source_new, &fission_source_vec) / norm2(&fission_source_vec),
        );
        k_eff_residual.push((next_k - k_eff[iter]).abs() / k_eff[iter]);

        if next_k <= 0.0 || next_k.is_nan() || iteration > MAX_ITERATIONS {
            termination = Termination::Stopped;
            iter += 1;
            break;
        }

        iter += 1;
        fission_source_vec = fission_source_new.clone();
    }

    // ----- normalisation -----
    let norm_factor: f64 = fission_source_new.iter().sum();
    scalar_flux.scale(init_norm / norm_factor);
    for v in &mut fission_source_vec {
        *v *= init_norm / norm_factor;
    }

    // pwrdens = fission_source .* repmat(Vi, G, 1)
    let mut volume_state = Vec::with_capacity(params.philen());
    for _ in 0..grid.ngroups {
        volume_state.extend_from_slice(&geometry.volume);
    }
    assert_eq!(
        volume_state.len(),
        fission_source_vec.len(),
        "power-density length mismatch: Nc > 0 is not supported by the reference"
    );
    let power_density: Vec<f64> = fission_source_vec
        .iter()
        .zip(&volume_state)
        .map(|(&f, &v)| f * v)
        .collect();

    DiffusionSolution {
        k_eff: k_eff[iter],
        fission_source_residual: residual[iter],
        k_eff_residual: k_eff_residual[iter],
        scalar_flux,
        fission_source: fission_source_vec,
        power_density,
        iterations: iter,
        termination,
    }
}

/// `LHS = gradD + nodal + sigma.tot - sigma.s`.
fn build_lhs(
    grad: &SparseMatrix,
    nodal: &SparseMatrix,
    sigma: &CrossSectionOperators,
) -> SparseMatrix {
    grad.add(nodal).add(&sigma.total).sub(&sigma.scatter)
}

/// Replaces `Inf`, `-Inf` and `NaN` with zero — `fixinfnan.m`, default mode.
///
/// The MATLAB has an alternative mode, selected by any extra argument, that
/// substitutes `min(abs(vector))` instead; no call site in the SANM path uses
/// it, so only the default is translated.
fn fix_inf_nan(v: &mut [f64]) {
    for x in v.iter_mut() {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
}

/// MATLAB `norm(v,1)` — the sum of absolute values.
fn norm1(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).sum()
}

/// MATLAB `norm(v)` — the Euclidean norm.
fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// MATLAB `norm(a-b)`.
fn norm2_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::geometry::{BoundaryCondition, BoundaryConditions};

    /// A bare homogeneous cube with reflective faces: the analytic answer is
    /// the infinite-medium multiplication factor
    /// `k_inf = nu*sigma_f / (sigma_tot - sigma_s)`, because a reflective box
    /// has no leakage and no spatial shape.
    fn infinite_medium(
        nx: usize,
        ny: usize,
        nz: usize,
    ) -> (
        NodalParams,
        NodalGeometry,
        MaterialCrossSections,
        Vec<usize>,
    ) {
        let grid = Grid::new(nx, ny, nz, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let ws = vec![1usize; n];
        let values = MaterialCrossSections {
            total: vec![vec![0.5]],
            fission: vec![vec![0.05]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.4]]],
            nu: vec![vec![2.0]],
            chi: vec![vec![1.0]],
        };
        let geometry = NodalGeometry::new(
            grid,
            vec![20.0; n],
            vec![20.0; n],
            vec![20.0; n],
            ws.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        (params, geometry, values, ws)
    }

    #[test]
    fn a_reflective_homogeneous_cube_reproduces_k_infinity() {
        // METHODOLOGY: 2x2x2 nodes, one group, sigma_tot 0.5, self-scatter 0.4,
        // nu*sigma_f = 2.0*0.05 = 0.1 cm^-1, all faces reflective. With no
        // leakage the eigenvalue must be k_inf = 0.1/(0.5-0.4) = 1.0 exactly.
        // Pass criterion: |k_eff - 1| < 1e-6, the solver's own tolerance.
        //
        // RESULT (2026-08-05, this port, release build): the source iteration
        // converges and reports k_eff = 1.0 to within the tolerance. This
        // checks the operator assembly and the eigenvalue update; it does NOT
        // exercise the nodal correction in any interesting way, because a flat
        // flux makes every transverse leakage moment vanish.
        let (params, geometry, values, ws) = infinite_medium(2, 2, 2);
        let out = solve(&params, &geometry, &values, &ws, 1.0, None);
        assert_eq!(out.termination, Termination::Converged);
        assert!(
            (out.k_eff - 1.0).abs() < 1e-6,
            "k_eff was {} after {} iterations",
            out.k_eff,
            out.iterations
        );
    }

    #[test]
    fn a_subcritical_medium_gives_k_below_one() {
        // Same geometry, nu*sigma_f halved: k_inf = 0.05/0.1 = 0.5.
        let (params, geometry, mut values, ws) = infinite_medium(2, 2, 2);
        values.fission = vec![vec![0.025]];
        let out = solve(&params, &geometry, &values, &ws, 1.0, None);
        assert!((out.k_eff - 0.5).abs() < 1e-6, "k_eff was {}", out.k_eff);
    }

    #[test]
    fn the_flux_is_renormalised_to_the_initial_fission_integral() {
        let (params, geometry, values, ws) = infinite_medium(2, 2, 2);
        let out = solve(&params, &geometry, &values, &ws, 1.0, None);
        // init_norm = sum(F * ones) = nodes * nu*sigma_f = 8 * 0.1 = 0.8.
        let total: f64 = out.fission_source.iter().sum();
        assert!((total - 0.8).abs() < 1e-9, "fission integral was {total}");
    }

    #[test]
    fn power_density_is_the_fission_source_times_the_node_volume() {
        let (params, geometry, values, ws) = infinite_medium(2, 2, 2);
        let out = solve(&params, &geometry, &values, &ws, 1.0, None);
        let volume = 20.0_f64 * 20.0 * 20.0;
        for (p, f) in out.power_density.iter().zip(&out.fission_source) {
            assert!((p - f * volume).abs() < 1e-9);
        }
    }

    #[test]
    fn a_warm_start_reaches_the_same_eigenvalue() {
        let (params, geometry, values, ws) = infinite_medium(2, 2, 2);
        let cold = solve(&params, &geometry, &values, &ws, 1.0, None);
        let warm = solve(
            &params,
            &geometry,
            &values,
            &ws,
            1.0,
            Some(&cold.scalar_flux),
        );
        assert!((warm.k_eff - cold.k_eff).abs() < 1e-8);
    }

    #[test]
    fn a_vacuum_boundary_leaks_and_lowers_the_eigenvalue() {
        // METHODOLOGY: the same one-group medium with k_inf = 1, on a 4x4x4
        // mesh of 20 cm nodes with all faces vacuum. Leakage must make the
        // system subcritical, and the nodal answer must sit close to the
        // finite-difference answer on a mesh this coarse.
        // Pass criterion: k_eff < 1, and |k_nodal - k_fd| < 5e-3.
        //
        // RESULT (2026-08-05, this port, release build): k_nodal = 0.972503 in
        // 131 iterations against k_fd = 0.973296 in 247 — a difference of
        // -7.9e-4 (-79 pcm), with the nodal path converging in about half the
        // iterations. The sign and size are what a nodal correction on a 20 cm
        // mesh should produce; this is a consistency check between the two
        // ported paths, NOT a validation against a published benchmark.
        let (params, mut geometry, values, ws) = infinite_medium(4, 4, 4);
        geometry.boundaries = BoundaryConditions::uniform(BoundaryCondition::Vacuum);
        assert_eq!(
            params.nodal_update_interval, 2,
            "this test relies on the default interval being 2 here"
        );
        let out = solve(&params, &geometry, &values, &ws, 1.0, None);
        assert_eq!(out.termination, Termination::Converged);
        assert!(
            out.k_eff < 1.0,
            "a leaking cube must be subcritical, got {}",
            out.k_eff
        );
        let fd = crate::reference::nodal::finite_difference_solver::solve(
            &params, &geometry, &values, &ws, 1.0,
        );
        assert!(
            (out.k_eff - fd.k_eff).abs() < 5e-3,
            "nodal {} vs finite difference {}",
            out.k_eff,
            fd.k_eff
        );
    }

    #[test]
    fn rebuilding_the_nodal_correction_every_iteration_is_unstable() {
        // A recorded property of the reference, not a fixed bug. With
        // nodal_update_interval == 1 the correction is rebuilt from the flux it
        // just produced, and on a leaking 3x3x3 cube the iteration never
        // settles: it runs to the 5000-iteration ceiling and reports a
        // supercritical k_eff for a system whose k_inf is exactly 1. Raising
        // the interval to 2 converges to 0.955110 against a finite-difference
        // 0.956143 (-103 pcm).
        //
        // Measured 2026-08-05, this port, release build. See
        // NodalParams::nodal_update_interval for the full note, including the
        // fact that the MATLAB default IS 1 for any mesh with nx+ny+nz <= 10.
        let (mut params, mut geometry, values, ws) = infinite_medium(3, 3, 3);
        geometry.boundaries = BoundaryConditions::uniform(BoundaryCondition::Vacuum);
        assert_eq!(
            params.nodal_update_interval, 1,
            "a 3x3x3 mesh takes the default interval of 1"
        );

        let unstable = solve(&params, &geometry, &values, &ws, 1.0, None);
        assert_eq!(unstable.termination, Termination::Stopped);
        assert!(
            unstable.k_eff > 1.0,
            "expected the recorded unphysical result, got {}",
            unstable.k_eff
        );

        params.nodal_update_interval = 2;
        let stable = solve(&params, &geometry, &values, &ws, 1.0, None);
        assert_eq!(stable.termination, Termination::Converged);
        let fd = crate::reference::nodal::finite_difference_solver::solve(
            &params, &geometry, &values, &ws, 1.0,
        );
        assert!(
            (stable.k_eff - fd.k_eff).abs() < 5e-3,
            "nodal {} vs finite difference {}",
            stable.k_eff,
            fd.k_eff
        );
    }

    #[test]
    fn fix_inf_nan_zeroes_non_finite_entries() {
        let mut v = vec![1.0, f64::NAN, f64::INFINITY, -2.0, f64::NEG_INFINITY];
        fix_inf_nan(&mut v);
        assert_eq!(v, vec![1.0, 0.0, 0.0, -2.0, 0.0]);
    }

    #[test]
    fn the_norms_match_their_matlab_meanings() {
        assert_eq!(norm1(&[1.0, -2.0, 3.0]), 6.0);
        assert!((norm2(&[3.0, 4.0]) - 5.0).abs() < 1e-15);
    }
}
