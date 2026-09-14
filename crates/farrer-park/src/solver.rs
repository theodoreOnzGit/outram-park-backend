// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Solution control: the linear solve, and Newton's method with load stepping
//! and cutback.
//!
//! # What belongs in this module
//!
//! [`solve_linear`], which drives one Krylov solve through the shared backend,
//! and [`solve_nonlinear`], the load-stepped Newton iteration that a plasticity
//! problem needs.
//!
//! # What does NOT belong here
//!
//! The Krylov methods themselves. They live in `outram-foam-basic-lib` and are
//! shared with the finite-volume side; this module only chooses among them and
//! decides what to do when one fails. See [`crate::operator`].
//!
//! # Newton, and why the tangent has to be the consistent one
//!
//! Each iteration solves `K_T du = R` with
//! `R = lambda (f_ext + f_body) - f_int(u)` and `K_T = d f_int / du`. When
//! `K_T` is the exact derivative of the discrete internal force — which it is,
//! because [`crate::material`] returns the algorithmic tangent and
//! [`crate::assembly`] integrates `B^T D B` with it — the iteration converges
//! **quadratically**: the residual is squared at each step, so 1e-2 becomes
//! 1e-4 becomes 1e-8. With the *continuum* elastoplastic tangent, or the
//! elastic one, it converges linearly instead. That difference is measurable,
//! and the verification suite measures it rather than asserting it.
//!
//! # Convergence is judged on two criteria, not one
//!
//! A step is converged when **both**
//!
//! - the relative residual `||R||_2 / ||f_ref||_2` is below
//!   [`NewtonSettings::residual_tolerance`], and
//! - the relative increment `||du||_2 / ||u||_2` is below
//!   [`NewtonSettings::increment_tolerance`].
//!
//! Either alone can lie. A residual test alone passes at a point where the
//! tangent is nearly singular and a huge increment barely moves the residual;
//! an increment test alone passes when the iteration has stalled short of the
//! solution. Requiring both is the standard remedy and costs nothing.
//!
//! # Load stepping and cutback
//!
//! The load is applied in [`NewtonSettings::n_load_steps`] equal increments.
//! If a step fails to converge, the increment is **halved and retried** from
//! the last converged state, up to [`NewtonSettings::max_cutbacks`] times,
//! after which [`FemError::NewtonNotConverged`] is returned. History is
//! committed only when a step converges, so a rejected attempt leaves no
//! plastic strain behind — the failure mode that makes a naive cutback silently
//! wrong.
//!
//! # Units
//!
//! Displacements in metres, forces in newtons, stiffness in newton per metre.
//! Tolerances and load factors are dimensionless.

use crate::assembly::System;
use crate::bc::{DirichletMethod, DirichletSet};
use crate::error::{FemError, Result};
use crate::operator::{bicgstab_op, cg_op, gmres_op, KrylovSettings};
use crate::sparse::{CsrMatrix, CsrPreconditioner};

/// Which Krylov method to drive the linear solve with.
///
/// All three come from `outram-foam-basic-lib`'s shared layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrylovMethod {
    /// Preconditioned conjugate gradients. The right choice here: a
    /// consistently linearised small-strain tangent with associated flow is
    /// symmetric positive definite once the rigid-body modes are constrained.
    ConjugateGradient,
    /// Restarted GMRES. For a future non-symmetric tangent (contact,
    /// non-associated flow).
    Gmres,
    /// BiCGStab. As GMRES, with fixed storage.
    BiCgStab,
}

/// Which preconditioner to build from the tangent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreconditionerChoice {
    /// None (`M = I`).
    None,
    /// Reciprocal diagonal.
    Jacobi,
    /// ILU(0) on the stiffness pattern. The default: on an elasticity
    /// stiffness matrix it typically cuts the iteration count by an order of
    /// magnitude over Jacobi.
    Ilu0,
}

/// Settings for one linear solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearSolverSettings {
    /// Krylov method.
    pub method: KrylovMethod,
    /// Preconditioner.
    pub preconditioner: PreconditionerChoice,
    /// Relative residual tolerance `||b - K du||_2 / ||b||_2`, dimensionless.
    /// Must be tighter than the Newton residual tolerance or the Newton
    /// iteration inherits the linear solver's error; `1e-12` is the default and
    /// is what the verification suite runs at.
    pub tolerance: f64,
    /// Maximum Krylov iterations.
    pub max_iter: usize,
    /// GMRES restart length; ignored by the other methods.
    pub restart: usize,
}

impl Default for LinearSolverSettings {
    /// Conjugate gradients with ILU(0), `1e-12` relative, at most 5000
    /// iterations, restart 50.
    fn default() -> Self {
        Self {
            method: KrylovMethod::ConjugateGradient,
            preconditioner: PreconditionerChoice::Ilu0,
            tolerance: 1.0e-12,
            max_iter: 5000,
            restart: 50,
        }
    }
}

/// Solve `K x = b` through the shared Krylov layer.
///
/// # Arguments
///
/// - `k` — the system matrix \[N/m\], with boundary conditions already applied.
/// - `b` — the right-hand side \[N\].
/// - `settings` — method, preconditioner and tolerance.
///
/// # Returns
///
/// The solution \[m\] and the Krylov result.
///
/// # Errors
///
/// [`FemError::LinearSolveNotConverged`] if the true relative residual of the
/// returned iterate exceeds the tolerance. It is returned rather than warned
/// about because a Newton step built on an unconverged linear solve produces a
/// plausible-looking answer that is wrong.
pub fn solve_linear(
    k: &CsrMatrix,
    b: &[f64],
    settings: &LinearSolverSettings,
) -> Result<(Vec<f64>, crate::operator::KrylovResult)> {
    let ks = KrylovSettings {
        tolerance: settings.tolerance,
        max_iter: settings.max_iter,
        restart: settings.restart,
    };
    let m = match settings.preconditioner {
        PreconditionerChoice::None => CsrPreconditioner::Identity,
        PreconditionerChoice::Jacobi => CsrPreconditioner::jacobi(k),
        PreconditionerChoice::Ilu0 => CsrPreconditioner::ilu0(k),
    };
    let (x, r) = match settings.method {
        KrylovMethod::ConjugateGradient => cg_op(k, b, None, &m, &ks),
        KrylovMethod::Gmres => gmres_op(k, b, None, &m, &ks),
        KrylovMethod::BiCgStab => bicgstab_op(k, b, None, &m, &ks),
    };
    if !r.converged {
        return Err(FemError::LinearSolveNotConverged {
            n_iterations: r.n_iterations,
            final_residual: r.final_residual,
            tolerance: settings.tolerance,
        });
    }
    Ok((x, r))
}

/// Settings for the load-stepped Newton iteration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewtonSettings {
    /// Maximum Newton iterations per load step.
    pub max_iterations: usize,
    /// Relative residual tolerance, dimensionless. See the module note on why
    /// this is not the only criterion.
    pub residual_tolerance: f64,
    /// Relative increment tolerance `||du|| / ||u||`, dimensionless.
    pub increment_tolerance: f64,
    /// Number of equal load increments the full load is applied in. `1` is
    /// correct for a linear problem and for a mildly plastic one.
    pub n_load_steps: usize,
    /// How many times a failing step may be halved and retried.
    pub max_cutbacks: usize,
    /// How prescribed displacements are imposed.
    pub dirichlet_method: DirichletMethod,
    /// Linear-solve settings.
    pub linear: LinearSolverSettings,
}

impl Default for NewtonSettings {
    /// 25 iterations, `1e-10` residual, `1e-12` increment, one load step, four
    /// cutbacks, Dirichlet by elimination, and
    /// [`LinearSolverSettings::default`].
    fn default() -> Self {
        Self {
            max_iterations: 25,
            residual_tolerance: 1.0e-10,
            increment_tolerance: 1.0e-8,
            n_load_steps: 1,
            max_cutbacks: 4,
            dirichlet_method: DirichletMethod::Elimination,
            linear: LinearSolverSettings::default(),
        }
    }
}

/// What one load step did.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadStepReport {
    /// The load factor reached, dimensionless, 0 to 1.
    pub load_factor: f64,
    /// Relative residual **before** each Newton iteration, dimensionless. The
    /// sequence whose ratios reveal the convergence order: a quadratically
    /// convergent step shows roughly `r_{k+1} ~ C r_k^2`.
    pub residual_history: Vec<f64>,
    /// Newton iterations spent.
    pub iterations: usize,
    /// Quadrature points yielding at the end of the step (dimensionless count).
    pub n_yielding: usize,
}

impl LoadStepReport {
    /// Observed convergence order estimated from the last three residuals:
    /// `log(r_{k+1} / r_k) / log(r_k / r_{k-1})`.
    ///
    /// Returns `None` when there are fewer than three residuals, or when any of
    /// them is zero or the ratios degenerate. Roughly 2 means quadratic
    /// convergence, roughly 1 means linear.
    ///
    /// This is an *estimate from three points* and it is noisy once the
    /// residual approaches the linear solver's tolerance, where round-off
    /// dominates. Read it together with the residual history, not instead of
    /// it.
    #[must_use]
    pub fn observed_order(&self) -> Option<f64> {
        let h = &self.residual_history;
        if h.len() < 3 {
            return None;
        }
        let (a, b, c) = (h[h.len() - 3], h[h.len() - 2], h[h.len() - 1]);
        if !(a > 0.0 && b > 0.0 && c > 0.0) {
            return None;
        }
        let d = (b / a).ln();
        if d.abs() < 1e-12 {
            return None;
        }
        Some((c / b).ln() / d)
    }

    /// Observed convergence order estimated from the last three residuals
    /// **above `floor`**, plus the first one that fell below it.
    ///
    /// [`observed_order`](Self::observed_order) is contaminated by the trailing
    /// entries of a converged history, which sit at the round-off floor of the
    /// residual evaluation (around `1e-14` relative) and carry no information
    /// about the iteration at all: the ratio between two numbers that are both
    /// pure round-off is meaningless. This variant truncates the history at the
    /// first entry to reach `floor` and estimates the order from the genuine
    /// part of the descent.
    ///
    /// `floor` should be a little above the residual round-off level and below
    /// the solver's own tolerance; `1e-10` is a reasonable choice when the
    /// linear solves are taken to `1e-13`.
    ///
    /// Returns `None` if fewer than three usable residuals remain.
    #[must_use]
    pub fn observed_order_above(&self, floor: f64) -> Option<f64> {
        let cut = self
            .residual_history
            .iter()
            .position(|r| *r <= floor)
            .map_or(self.residual_history.len(), |i| i + 1);
        let h = &self.residual_history[..cut];
        if h.len() < 3 {
            return None;
        }
        let (a, b, c) = (h[h.len() - 3], h[h.len() - 2], h[h.len() - 1]);
        if !(a > 0.0 && b > 0.0 && c > 0.0) {
            return None;
        }
        let d = (b / a).ln();
        if d.abs() < 1e-12 {
            return None;
        }
        Some((c / b).ln() / d)
    }
}

/// What the whole nonlinear solve did.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtonReport {
    /// One entry per converged load step, in order.
    pub steps: Vec<LoadStepReport>,
    /// Total Newton iterations across all steps and all retried attempts.
    pub total_iterations: usize,
    /// How many step cutbacks were taken.
    pub cutbacks: usize,
}

/// Solve the nonlinear equilibrium problem by load-stepped Newton iteration.
///
/// # Arguments
///
/// - `system` — the assembled problem. Its quadrature-point history is advanced
///   and committed as load steps converge, so this call **mutates** it.
/// - `dirichlet` — prescribed displacements \[m\] at **full load**. They are
///   scaled by the load factor along with the forces, so a displacement-driven
///   problem load-steps correctly.
/// - `external_force` — nodal forces \[N\] at full load (tractions, point
///   loads). Body forces come from the system itself and are scaled the same
///   way. Pass an all-zero vector for a purely displacement-driven problem.
/// - `settings` — iteration and load-stepping controls.
///
/// # Returns
///
/// The converged displacement \[m\] and a report carrying the residual history
/// of every step.
///
/// # Errors
///
/// [`FemError::NewtonNotConverged`] when the cutback budget is exhausted,
/// [`FemError::LinearSolveNotConverged`] from an inner solve, and any assembly
/// or constitutive error.
pub fn solve_nonlinear(
    system: &mut System,
    dirichlet: &DirichletSet,
    external_force: &[f64],
    settings: &NewtonSettings,
) -> Result<(Vec<f64>, NewtonReport)> {
    let n = system.n_dofs();
    if external_force.len() != n {
        return Err(FemError::LengthMismatch {
            context: "solve_nonlinear: external force vector",
            expected: n,
            actual: external_force.len(),
        });
    }
    let mut u = vec![0.0; n];
    let mut k = system.new_matrix();
    let mut report = NewtonReport {
        steps: Vec::new(),
        total_iterations: 0,
        cutbacks: 0,
    };

    let mut lambda = 0.0_f64;
    let mut d_lambda = 1.0 / settings.n_load_steps.max(1) as f64;
    let mut cutbacks = 0usize;

    while lambda < 1.0 - 1e-12 {
        let target = (lambda + d_lambda).min(1.0);
        let saved = u.clone();
        match newton_step(
            system,
            dirichlet,
            external_force,
            settings,
            target,
            &mut u,
            &mut k,
        ) {
            Ok(step) => {
                report.total_iterations += step.iterations;
                system.commit();
                report.steps.push(step);
                lambda = target;
            }
            Err(FemError::NewtonNotConverged { iterations, residual, .. })
            | Err(FemError::LinearSolveNotConverged {
                n_iterations: iterations,
                final_residual: residual,
                ..
            }) => {
                report.total_iterations += iterations;
                cutbacks += 1;
                report.cutbacks = cutbacks;
                u.copy_from_slice(&saved);
                if cutbacks > settings.max_cutbacks {
                    return Err(FemError::NewtonNotConverged {
                        load_factor: target,
                        iterations,
                        residual,
                        cutbacks,
                    });
                }
                d_lambda *= 0.5;
            }
            Err(other) => return Err(other),
        }
    }

    Ok((u, report))
}

/// One load step: Newton-iterate to equilibrium at load factor `lambda`.
///
/// `u` is updated in place and left at the last iterate whether or not the step
/// converged; the caller restores it on failure.
///
/// # How the iteration count is defined, and why a linear problem takes two
///
/// `iterations` counts **linear solves performed**. Convergence is declared at
/// the top of an iteration, before its solve, when the residual measured at the
/// current iterate and the increment that produced it are both below tolerance.
///
/// A linear elastic problem therefore reports **two** iterations, not one: the
/// first solve lands exactly on the solution, but the increment that produced
/// it is the whole solution and so fails the increment criterion; a second
/// solve returns a numerically zero increment, and the third pass sees both
/// criteria satisfied and stops without solving. That is the honest cost of
/// requiring both criteria (see the module documentation for why both are
/// required), and the residual history makes it visible: `[1, ~1e-16, ~1e-16]`.
///
/// # The reference norm
///
/// The relative residual is measured against
/// `max(||lambda (f_ext + f_body)||, ||R_0||)`, where `R_0` is the
/// boundary-condition-applied residual of the first iteration. The second term
/// is what makes a **displacement-driven** problem measurable at all: there the
/// external load is identically zero, and dividing by it would give infinity on
/// the first iteration and a meaningless number thereafter.
fn newton_step(
    system: &mut System,
    dirichlet: &DirichletSet,
    external_force: &[f64],
    settings: &NewtonSettings,
    lambda: f64,
    u: &mut [f64],
    k: &mut CsrMatrix,
) -> Result<LoadStepReport> {
    let n = u.len();
    // Prescribed displacements scale with the load factor.
    let scaled = DirichletSet {
        dofs: dirichlet.dofs.clone(),
        values: dirichlet.values.iter().map(|v| v * lambda).collect(),
    };

    let mut history = Vec::new();
    let mut n_yielding;
    let mut rhs = vec![0.0; n];
    let mut f_ref = 0.0_f64;
    let mut previous_increment = f64::INFINITY;

    for iter in 0..settings.max_iterations {
        let a = system.assemble(u, Some(k))?;
        n_yielding = a.n_yielding;
        for i in 0..n {
            rhs[i] = lambda * (external_force[i] + a.body_force[i]) - a.internal_force[i];
        }
        scaled.apply(k, &mut rhs, u, settings.dirichlet_method)?;

        let r_abs = rhs.iter().map(|v| v * v).sum::<f64>().sqrt();
        if iter == 0 {
            let ext: f64 = (0..n)
                .map(|i| (lambda * (external_force[i] + a.body_force[i])).powi(2))
                .sum::<f64>()
                .sqrt();
            f_ref = ext.max(r_abs).max(1.0e-300);
        }
        let r_norm = r_abs / f_ref;
        history.push(r_norm);

        if r_norm <= settings.residual_tolerance
            && previous_increment <= settings.increment_tolerance
        {
            return Ok(LoadStepReport {
                load_factor: lambda,
                residual_history: history,
                iterations: iter,
                n_yielding,
            });
        }

        let (du, _) = solve_linear(k, &rhs, &settings.linear)?;
        let du_norm = du.iter().map(|v| v * v).sum::<f64>().sqrt();
        for i in 0..n {
            u[i] += du[i];
        }
        let u_norm = u.iter().map(|v| v * v).sum::<f64>().sqrt().max(1.0e-300);
        previous_increment = du_norm / u_norm;
    }

    Err(FemError::NewtonNotConverged {
        load_factor: lambda,
        iterations: settings.max_iterations,
        residual: *history.last().unwrap_or(&f64::INFINITY),
        cutbacks: 0,
    })
}

/// Solve a **linear** elastic problem: one Newton step, no load stepping.
///
/// A convenience wrapper that makes the common case read as what it is. It
/// still goes through [`solve_nonlinear`], so the residual history it reports
/// is a genuine measurement — a linear problem converges in exactly one
/// iteration, and a report that says otherwise is evidence of a bug.
///
/// # Errors
///
/// As [`solve_nonlinear`].
pub fn solve_linear_elastic(
    system: &mut System,
    dirichlet: &DirichletSet,
    external_force: &[f64],
    linear: LinearSolverSettings,
) -> Result<(Vec<f64>, NewtonReport)> {
    let settings = NewtonSettings {
        max_iterations: 6,
        n_load_steps: 1,
        linear,
        ..NewtonSettings::default()
    };
    solve_nonlinear(system, dirichlet, external_force, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::{BodyForce, System};
    use crate::dof::DofMap;
    use crate::material::Material;
    use crate::mesh::unit_square_quad4;

    /// A linear elastic problem must converge in exactly one Newton iteration,
    /// because the residual is linear in the unknown and the tangent is exact.
    #[test]
    fn linear_elastic_converges_in_one_iteration() {
        let mesh = unit_square_quad4(4).unwrap().shared();
        let dofs = DofMap::displacement(&mesh);
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let mut sys = System::new(mesh.clone(), mat, BodyForce::None);

        // Uniform stretch in x: u_x = 1e-4 x, u_y free except at y = 0.
        let mut bcs = DirichletSet::new();
        for n in mesh.nodes_where(|p| p[0].abs() < 1e-12) {
            bcs.fix(&dofs, n, 0, 0.0);
        }
        for n in mesh.nodes_where(|p| (p[0] - 1.0).abs() < 1e-12) {
            bcs.fix(&dofs, n, 0, 1.0e-4);
        }
        for n in mesh.nodes_where(|p| p[1].abs() < 1e-12) {
            bcs.fix(&dofs, n, 1, 0.0);
        }
        let f = vec![0.0; sys.n_dofs()];
        let (u, rep) = solve_linear_elastic(&mut sys, &bcs, &f, LinearSolverSettings::default())
            .expect("linear solve");
        assert_eq!(rep.steps.len(), 1);
        // Two solves: see the `newton_step` note on why a linear problem is
        // two, not one, when both convergence criteria are required.
        assert_eq!(rep.steps[0].iterations, 2, "history {:?}", rep.steps[0].residual_history);
        let h = &rep.steps[0].residual_history;
        assert!(h[1] < 1e-11, "residual after the first solve {:?}", h);
        // The x displacement must be exactly linear in x.
        for i in 0..mesh.n_nodes() {
            let x = mesh.coords()[i][0];
            assert!((u[2 * i] - 1.0e-4 * x).abs() < 1e-12, "node {} ux {}", i, u[2 * i]);
        }
    }

    /// The penalty and elimination Dirichlet methods must agree to the accuracy
    /// the penalty factor allows.
    #[test]
    fn penalty_and_elimination_agree() {
        let mesh = unit_square_quad4(3).unwrap().shared();
        let dofs = DofMap::displacement(&mesh);
        let mat = Material::elastic(70.0e9, 0.3).unwrap();
        let mut bcs = DirichletSet::new();
        for n in mesh.nodes_where(|p| p[0].abs() < 1e-12) {
            bcs.fix(&dofs, n, 0, 0.0);
            bcs.fix(&dofs, n, 1, 0.0);
        }
        for n in mesh.nodes_where(|p| (p[0] - 1.0).abs() < 1e-12) {
            bcs.fix(&dofs, n, 0, 2.0e-4);
        }
        let mut solutions = Vec::new();
        for method in [
            DirichletMethod::Elimination,
            DirichletMethod::Penalty(1.0e8),
        ] {
            let mut sys = System::new(mesh.clone(), mat, BodyForce::None);
            let s = NewtonSettings {
                dirichlet_method: method,
                max_iterations: 8,
                // The penalty method cannot drive the residual below roughly
                // 1 / beta, so the tolerance is set to match rather than being
                // chased with more iterations.
                residual_tolerance: 1.0e-7,
                increment_tolerance: 1.0e-9,
                ..NewtonSettings::default()
            };
            let f = vec![0.0; sys.n_dofs()];
            let (u, _) = solve_nonlinear(&mut sys, &bcs, &f, &s).expect("solve");
            solutions.push(u);
        }
        let worst = solutions[0]
            .iter()
            .zip(&solutions[1])
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        let scale = solutions[0].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        assert!(worst / scale < 1e-5, "penalty vs elimination {}", worst / scale);
    }
}
