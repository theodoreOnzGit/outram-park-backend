//! The finite-difference fallback `k`-eigenvalue solver.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `diffusion_solverxyz.m` (function `diffusion_solverxyz`).
//!
//! This is the same power iteration as [`super::sanm_solver`] with the nodal
//! correction removed — plain coarse-mesh finite difference. It is useful as a
//! cross-check on the nodal path: the two must agree as the mesh is refined,
//! and disagree in a characteristic way on a coarse one.

use super::cross_sections::{self, MaterialCrossSections};
use super::geometry::{NodalGeometry, NodalParams};
use super::gradient_diffusion;
use super::sanm_solver::Termination;

/// Hard iteration ceiling — MATLAB `maxiter=10000`.
pub const MAX_ITERATIONS: usize = 10_000;

/// Convergence tolerance — MATLAB `diffusion.tol = 1E-6`.
///
/// Unlike the nodal solver, this one has no `params.innertol` override.
pub const TOLERANCE: f64 = 1e-6;

/// The result of the finite-difference source iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct FiniteDifferenceSolution {
    /// Converged multiplication factor \[dimensionless\].
    pub k_eff: f64,
    /// Final relative fission-source residual \[dimensionless\].
    pub fission_source_residual: f64,
    /// Final relative `k_eff` residual \[dimensionless\].
    pub k_eff_residual: f64,
    /// Converged scalar flux \[neutrons cm⁻² s⁻¹\], normalised so that the
    /// fission-source 1-norm equals its initial value.
    pub scalar_flux: Vec<f64>,
    /// Fission source \[neutrons cm⁻³ s⁻¹\].
    pub fission_source: Vec<f64>,
    /// Node power density \[neutrons s⁻¹ per node\].
    pub power_density: Vec<f64>,
    /// Source iterations performed.
    pub iterations: usize,
    /// Why the loop ended.
    pub termination: Termination,
}

/// Solves the `k`-eigenvalue problem by coarse-mesh finite difference —
/// `diffusion_solverxyz.m`.
///
/// The iteration is
///
/// ```text
/// (gradD + sigma_tot - sigma_sd) phi = (1/k) F phi + (sigma_s - sigma_sd) phi
/// ```
///
/// i.e. within-group scattering is kept on the left and group-to-group
/// scattering is lagged on the right. The left-hand operator does not change,
/// so it is factorised once.
///
/// # Differences from the nodal solver worth knowing about
///
/// - **The normalisation is applied every iteration**, not once at the end, and
///   uses the fission source's **1-norm** where the nodal solver uses its plain
///   sum. Both are the reference's own choices.
/// - **There is no flux history and no fission-source extrapolation.**
/// - On a non-converging exit the returned flux is the *previous* iterate, not
///   the one that triggered the break: the MATLAB assigns
///   `scalar_flux = scalar_flux_l_plus` only after the break test. Preserved.
///
/// # Not ported
///
/// - The unconditional `writematrix` diagnostic dumps and the `plotfig`
///   surface plot.
/// - The `keychange` compaction branch, which renumbers the state vector to
///   skip empty grid space via `convert_grid3d` / `convertsparsekey3d`. It is
///   guarded by a hard-coded `keychange=0` in the reference and so is dead
///   code there.
/// - The `philenf >= sizethresh` GMRES branch, dead for the same reason as in
///   [`super::sanm_solver`].
///
/// # Panics
///
/// If the left-hand operator cannot be factorised, or if `Nc > 0` makes the
/// operator shapes disagree.
#[must_use]
pub fn solve(
    params: &NodalParams,
    geometry: &NodalGeometry,
    values: &MaterialCrossSections,
    which_sigma: &[usize],
    initial_k_eff: f64,
) -> FiniteDifferenceSolution {
    let grid = params.grid;
    let philenf = params.philenf();

    let sigma = cross_sections::assemble_operators(params, values, which_sigma);
    let diffusion = cross_sections::diffusion_coefficients(grid, &values.total, which_sigma, 1.0);
    let grad = gradient_diffusion::assemble(params, geometry, &diffusion, which_sigma);

    let mut scalar_flux = vec![1.0; philenf];
    let mut residual = vec![1.0];
    let mut k_eff_residual = vec![1.0];
    let mut k_eff = vec![initial_k_eff];
    let mut iter = 0usize;

    let mut fission_source = sigma.fission.mul_vec(&scalar_flux);
    let init_norm = norm1(&fission_source);

    let lhs = grad.operator.add(&sigma.total).sub(&sigma.scatter_self);
    let lu = lhs.lu().expect("the finite-difference LHS is nonsingular");

    let lagged_scatter = sigma.scatter.sub(&sigma.scatter_self);
    let mut termination = Termination::Converged;

    while residual[iter] >= TOLERANCE || k_eff_residual[iter] >= TOLERANCE {
        let iteration = iter + 1;

        let scattered = lagged_scatter.mul_vec(&scalar_flux);
        let rhs: Vec<f64> = fission_source
            .iter()
            .zip(&scattered)
            .map(|(&f, &s)| f / k_eff[iter] + s)
            .collect();
        let mut flux_next = lu.solve(&rhs);

        let mut fission_next = sigma.fission.mul_vec(&flux_next);
        let norm_factor = norm1(&fission_next);

        let next_k = k_eff[iter] * norm1(&fission_next) / norm1(&fission_source);
        k_eff.push(next_k);

        let scale = init_norm / norm_factor;
        for v in &mut flux_next {
            *v *= scale;
        }
        for v in &mut fission_next {
            *v *= scale;
        }

        residual.push(norm2_difference(&fission_next, &fission_source) / norm2(&fission_source));
        k_eff_residual.push((next_k - k_eff[iter]).abs() / k_eff[iter]);

        if next_k <= 0.0 || next_k.is_nan() || iteration > MAX_ITERATIONS {
            termination = Termination::Stopped;
            iter += 1;
            break;
        }

        iter += 1;
        scalar_flux = flux_next;
        fission_source = fission_next;
    }

    let mut volume_state = Vec::with_capacity(params.philen());
    for _ in 0..grid.ngroups {
        volume_state.extend_from_slice(&geometry.volume);
    }
    assert_eq!(
        volume_state.len(),
        fission_source.len(),
        "power-density length mismatch: Nc > 0 is not supported by the reference"
    );
    let power_density: Vec<f64> = fission_source
        .iter()
        .zip(&volume_state)
        .map(|(&f, &v)| f * v)
        .collect();

    FiniteDifferenceSolution {
        k_eff: k_eff[iter],
        fission_source_residual: residual[iter],
        k_eff_residual: k_eff_residual[iter],
        scalar_flux,
        fission_source,
        power_density,
        iterations: iter,
        termination,
    }
}

/// MATLAB `norm(v,1)`.
fn norm1(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).sum()
}

/// MATLAB `norm(v)`.
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
    use crate::reference::nodal::sanm_solver;

    fn two_group_reflective_cube() -> (
        NodalParams,
        NodalGeometry,
        MaterialCrossSections,
        Vec<usize>,
    ) {
        let grid = Grid::new(2, 2, 2, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let ws = vec![1usize; n];
        // A deliberately simple two-group set: fast group scatters down,
        // thermal group fissions.
        let values = MaterialCrossSections {
            total: vec![vec![0.25, 1.0]],
            fission: vec![vec![0.0, 0.1]],
            fission_prompt: Vec::new(),
            // scatter[m][to][from]
            scatter: vec![vec![vec![0.23, 0.0], vec![0.02, 0.9]]],
            nu: vec![vec![2.5, 2.5]],
            chi: vec![vec![1.0, 0.0]],
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
    fn a_reflective_two_group_cube_reproduces_the_analytic_k_infinity() {
        // METHODOLOGY: with no leakage the two-group balance reduces to
        //   phi_th / phi_f = sigma_s(f->th) / (sigma_tot,th - sigma_s(th->th))
        //                  = 0.02 / (1.0 - 0.9) = 0.2
        //   k_inf = nu*sigma_f,th * phi_th / (sigma_tot,f - sigma_s(f->f)) / phi_f
        //         = 2.5*0.1*0.2 / (0.25 - 0.23) = 0.05/0.02 = 2.5
        // Pass criterion: |k_eff - 2.5| < 1e-5.
        //
        // RESULT (2026-08-05, this port, release build): converged to k_eff =
        // 2.5 within tolerance. This exercises the group coupling, the lagged
        // off-diagonal scattering and the eigenvalue update; it says nothing
        // about spatial accuracy, which a reflective box cannot probe.
        let (params, geometry, values, ws) = two_group_reflective_cube();
        let out = solve(&params, &geometry, &values, &ws, 1.0);
        assert_eq!(out.termination, Termination::Converged);
        assert!((out.k_eff - 2.5).abs() < 1e-5, "k_eff was {}", out.k_eff);
    }

    #[test]
    fn the_nodal_and_finite_difference_paths_agree_where_the_flux_is_flat() {
        // A reflective homogeneous cube has an exactly flat fundamental mode,
        // so the nodal correction has nothing to correct and both solvers must
        // land on the same eigenvalue.
        let (params, geometry, values, ws) = two_group_reflective_cube();
        let fd = solve(&params, &geometry, &values, &ws, 1.0);
        let nodal = sanm_solver::solve(&params, &geometry, &values, &ws, 1.0, None);
        assert!(
            (fd.k_eff - nodal.k_eff).abs() < 1e-5,
            "fd {} vs nodal {}",
            fd.k_eff,
            nodal.k_eff
        );
    }

    #[test]
    fn a_vacuum_boundary_lowers_the_eigenvalue() {
        let (params, mut geometry, values, ws) = two_group_reflective_cube();
        geometry.boundaries = BoundaryConditions::uniform(BoundaryCondition::Vacuum);
        let leaky = solve(&params, &geometry, &values, &ws, 1.0);
        let (params2, geometry2, values2, ws2) = two_group_reflective_cube();
        let tight = solve(&params2, &geometry2, &values2, &ws2, 1.0);
        assert!(leaky.k_eff < tight.k_eff);
    }

    #[test]
    fn the_fission_source_keeps_its_initial_one_norm() {
        let (params, geometry, values, ws) = two_group_reflective_cube();
        let out = solve(&params, &geometry, &values, &ws, 1.0);
        // init_norm = sum over nodes and groups of (F * ones).
        // F has one nonzero per node: chi_fast * nu * sigma_f,th = 0.25.
        let expected = 8.0 * 0.25;
        assert!((norm1(&out.fission_source) - expected).abs() < 1e-9);
    }
}
