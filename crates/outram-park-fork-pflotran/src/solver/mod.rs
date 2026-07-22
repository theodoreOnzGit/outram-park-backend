//! Newton–Krylov nonlinear solver layer (v1, KEYSTONE) — bead op-v6s.4.
//!
//! A serial, pure-Rust Newton driver for a nonlinear algebraic system
//! `F(x) = 0` (the discretized residual of a flow mode). It is **generic over
//! the system** via the [`NonlinearSystem`] contract — static dispatch, never a
//! trait object — and delegates each linear Newton step `J·dx = -F` to
//! `outram-foam-basic-lib`'s [`krylov`](outram_foam_basic_lib::krylov)
//! asymmetric solvers (BiCGStab / GMRES) wrapped by an ILU(0) / Jacobi /
//! identity preconditioner. No PETSc, no MPI.
//!
//! # Algorithm
//!
//! Given an initial guess `x` the driver iterates:
//!
//! 1. Assemble the residual `F(x)`; its 2-norm `||F||₂` measures how far `x` is
//!    from a root. The first iteration's norm is cached as `||F₀||₂`.
//! 2. **Converged** when `||F||₂ < abs_tol` OR `||F||₂ < rel_tol·||F₀||₂`.
//! 3. Otherwise assemble the Jacobian `J = dF/dx` into a fixed-sparsity
//!    [`LduMatrix`], build the chosen preconditioner from `J`, and solve
//!    `J·dx = -F` with the chosen Krylov method.
//! 4. **Line search (backtracking / Armijo):** try the full step `λ = 1`; if the
//!    residual norm is not sufficiently reduced, halve `λ` up to
//!    [`NewtonConfig::max_backtracks`] times, then update `x ← x + λ·dx`.
//!
//! The Jacobian sparsity (owner/neighbour face addressing) is **fixed** across
//! the whole solve; only the coefficient values change per iteration, so the
//! [`LduMatrix`] is allocated once and refilled in place.
//!
//! # Convergence & honesty
//!
//! If the loop exhausts [`NewtonConfig::max_iterations`] without meeting the
//! tolerance it returns [`PflotranError::Convergence`] carrying the last norm —
//! it never reports a non-root as a solution. A NaN/Inf appearing in the
//! residual or the Newton step is likewise reported as a `Convergence` failure
//! rather than propagated silently.

use crate::error::PflotranError;
use outram_foam_basic_lib::krylov::{
    self, KrylovSettings, Preconditioner,
};
use outram_foam_basic_lib::ldu_matrix::LduMatrix;

/// Compiler-enforced contract for a nonlinear system `F(x) = 0` solved by
/// Newton's method.
///
/// This trait is consumed via generics ([`NewtonSolver::solve`] takes
/// `S: NonlinearSystem`), so dispatch is **static** — it is never used as a
/// trait object (`Box<dyn _>` / `&dyn _`), per the workspace design rules.
///
/// # Sparsity contract
///
/// The Jacobian sparsity — the `(owner, neighbour)` face addressing returned by
/// [`ldu_addressing`](Self::ldu_addressing) — is **fixed for the entire solve**.
/// Only the coefficient *values* (filled by
/// [`assemble_jacobian`](Self::assemble_jacobian)) change from one Newton
/// iteration to the next. The driver relies on this to allocate the
/// [`LduMatrix`] once.
///
/// # Units
///
/// `x`, the residual, and the Jacobian entries are dimensionless numeric
/// coefficients here — any `uom` unit bookkeeping belongs to the flow mode that
/// assembles them, not to this generic contract.
pub trait NonlinearSystem {
    /// Number of degrees of freedom — the length of `x`, of the residual, and
    /// the `n_cells` of the assembled [`LduMatrix`]. Must be constant across the
    /// solve.
    fn n_dof(&self) -> usize;

    /// The fixed Jacobian sparsity as `(owner, neighbour)` arrays: one entry per
    /// off-diagonal face, with `owner[f] < neighbour[f]`, matching the
    /// addressing an [`LduMatrix`] is built from. Called once per solve.
    fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>);

    /// Assemble the residual `F(x)` into `out` (length [`n_dof`](Self::n_dof)).
    /// Newton drives this vector toward zero.
    ///
    /// `out` is a scratch buffer whose previous contents are overwritten; the
    /// implementation must write every entry.
    fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError>;

    /// Assemble the Jacobian `dF/dx` into `jac`, which is already sized to the
    /// addressing from [`ldu_addressing`](Self::ldu_addressing). The
    /// implementation must **zero** `jac.diag` / `jac.lower` / `jac.upper` and
    /// then fill them.
    ///
    /// Row = equation index, column = unknown index. For face `f` with owner
    /// `o` and neighbour `n`: `jac.diag[i]` holds `∂F_i/∂x_i`, `jac.upper[f]`
    /// holds `∂F_o/∂x_n`, and `jac.lower[f]` holds `∂F_n/∂x_o`.
    fn assemble_jacobian(&mut self, x: &[f64], jac: &mut LduMatrix) -> Result<(), PflotranError>;
}

/// Which Krylov method from [`outram_foam_basic_lib::krylov`] solves the linear
/// Newton step `J·dx = -F`.
///
/// Enum-dispatched (no trait objects): the driver matches on this to call
/// [`krylov::bicgstab`] or [`krylov::gmres`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearSolverKind {
    /// Stabilized bi-conjugate gradient — cheap per iteration, no restart
    /// parameter; the default for these nonsymmetric flow Jacobians.
    BiCGStab,
    /// Restarted GMRES — more robust for strongly nonsymmetric / stiff systems,
    /// at the cost of storing the restart-length Krylov basis.
    Gmres,
}

/// Which preconditioner wraps the Krylov solve, built afresh from the current
/// Jacobian each Newton iteration.
///
/// Enum-dispatched (no trait objects): the driver matches on this to construct
/// the corresponding [`Preconditioner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreconditionerKind {
    /// No preconditioning ([`Preconditioner::identity`]) — for testing or
    /// already-well-conditioned systems.
    Identity,
    /// Diagonal (Jacobi) scaling ([`Preconditioner::jacobi`]) — cheapest useful
    /// choice.
    Jacobi,
    /// Zero-fill incomplete-LU ([`Preconditioner::ilu0`]) — the strongest of the
    /// three and the default; good for diffusion-dominated Jacobians.
    Ilu0,
}

/// Newton solver configuration.
///
/// See [`NewtonConfig::default`] for the shipped defaults. Convergence is
/// declared when the residual 2-norm falls below `abs_tol`, or below
/// `rel_tol` times the initial residual norm.
pub struct NewtonConfig {
    /// Maximum number of Newton iterations before giving up with
    /// [`PflotranError::Convergence`].
    pub max_iterations: usize,
    /// Absolute convergence tolerance: converged when `||F||₂ < abs_tol`.
    pub abs_tol: f64,
    /// Relative convergence tolerance: converged when
    /// `||F||₂ < rel_tol · ||F₀||₂`, where `||F₀||₂` is the initial residual
    /// norm.
    pub rel_tol: f64,
    /// Which Krylov method solves each linear step.
    pub linear: LinearSolverKind,
    /// Which preconditioner wraps the Krylov solve.
    pub preconditioner: PreconditionerKind,
    /// Settings (tolerance / max_iter / restart) passed to the inner Krylov
    /// solve.
    pub linear_settings: KrylovSettings,
    /// Maximum backtracking line-search halvings per Newton step. `0` disables
    /// the line search entirely (pure full-step Newton, `λ = 1`).
    pub max_backtracks: usize,
}

impl Default for NewtonConfig {
    /// Defaults: 50 Newton iterations, `abs_tol = 1e-9`, `rel_tol = 1e-8`,
    /// BiCGStab + ILU(0), the default [`KrylovSettings`], and up to 10 line-search
    /// backtracks.
    fn default() -> Self {
        NewtonConfig {
            max_iterations: 50,
            abs_tol: 1e-9,
            rel_tol: 1e-8,
            linear: LinearSolverKind::BiCGStab,
            preconditioner: PreconditionerKind::Ilu0,
            linear_settings: KrylovSettings::default(),
            max_backtracks: 10,
        }
    }
}

/// Outcome of a [`NewtonSolver::solve`] call.
///
/// Returned by value on success; on failure the same information is summarized
/// inside the [`PflotranError::Convergence`] message.
#[derive(Debug, Clone)]
pub struct NewtonReport {
    /// Number of Newton iterations performed (residual evaluations at which the
    /// convergence test was applied).
    pub iterations: usize,
    /// Whether the tolerance was met. Always `true` in a successfully returned
    /// report — a `false` outcome is surfaced as an `Err` instead.
    pub converged: bool,
    /// The residual 2-norm `||F||₂` at exit.
    pub final_residual_norm: f64,
    /// The residual 2-norm at each iteration, in order — useful for plotting the
    /// (near-quadratic) Newton convergence tail.
    pub residual_history: Vec<f64>,
}

/// Serial pure-Rust Newton–Krylov nonlinear solver.
///
/// Construct with [`NewtonSolver::new`] and drive a [`NonlinearSystem`] to a
/// root with [`NewtonSolver::solve`]. The [`config`](Self::config) is public so
/// callers may inspect or tweak it between solves.
pub struct NewtonSolver {
    /// The Newton / line-search / linear-solver configuration.
    pub config: NewtonConfig,
}

impl NewtonSolver {
    /// Create a solver with the given configuration.
    pub fn new(config: NewtonConfig) -> Self {
        NewtonSolver { config }
    }

    /// Solve `F(x) = 0`, updating `x` in place with the converged root.
    ///
    /// On success returns a [`NewtonReport`] with the iteration count, final
    /// residual norm, and per-iteration residual history. Returns
    /// [`PflotranError::Convergence`] if the tolerance is not met within
    /// [`NewtonConfig::max_iterations`], if `x` has the wrong length, or if a
    /// NaN/Inf is encountered in the residual or the Newton step.
    ///
    /// The inner *linear* solve failing to converge is **not** treated as a hard
    /// error: the (best-effort) step direction is still used, since Newton is
    /// robust to inexact linear solves. Only a non-finite step is rejected.
    pub fn solve<S: NonlinearSystem>(
        &self,
        sys: &mut S,
        x: &mut [f64],
    ) -> Result<NewtonReport, PflotranError> {
        let n = sys.n_dof();
        if x.len() != n {
            return Err(PflotranError::InvalidInput(format!(
                "initial guess has length {} but the system has {n} DOF",
                x.len()
            )));
        }

        // Build the fixed-sparsity Jacobian matrix once (op-v6s.4 step 1).
        let (owner, neighbour) = sys.ldu_addressing();
        if owner.len() != neighbour.len() {
            return Err(PflotranError::InvalidInput(format!(
                "owner (len {}) and neighbour (len {}) addressing arrays differ",
                owner.len(),
                neighbour.len()
            )));
        }
        let mut jac = LduMatrix::new(n, owner, neighbour);

        // Scratch buffers reused across iterations.
        let mut f = vec![0.0_f64; n]; // residual F(x)
        let mut rhs = vec![0.0_f64; n]; // -F, the linear-solve right-hand side
        let mut x_trial = vec![0.0_f64; n]; // x + λ·dx during line search
        let mut f_trial = vec![0.0_f64; n]; // F(x_trial)

        let mut history: Vec<f64> = Vec::with_capacity(self.config.max_iterations + 1);
        let mut norm0: Option<f64> = None;

        for iter in 0..=self.config.max_iterations {
            // (op-v6s.4 step 2) residual + convergence test.
            sys.assemble_residual(x, &mut f)?;
            let norm = nrm2(&f);
            if !norm.is_finite() {
                return Err(PflotranError::Convergence(format!(
                    "residual norm is non-finite ({norm}) at Newton iteration {iter}"
                )));
            }
            history.push(norm);
            let base = *norm0.get_or_insert(norm);

            if norm < self.config.abs_tol || norm < self.config.rel_tol * base {
                return Ok(NewtonReport {
                    iterations: iter,
                    converged: true,
                    final_residual_norm: norm,
                    residual_history: history,
                });
            }

            // Out of iteration budget — no further step permitted.
            if iter == self.config.max_iterations {
                break;
            }

            // (op-v6s.4 step 3) assemble Jacobian, build preconditioner, solve
            // J·dx = -F.
            sys.assemble_jacobian(x, &mut jac)?;
            for i in 0..n {
                rhs[i] = -f[i];
            }

            let precond = match self.config.preconditioner {
                PreconditionerKind::Identity => Preconditioner::identity(),
                PreconditionerKind::Jacobi => Preconditioner::jacobi(&jac),
                PreconditionerKind::Ilu0 => Preconditioner::ilu0(&jac),
            };

            let (dx, _lin_result) = match self.config.linear {
                LinearSolverKind::BiCGStab => {
                    krylov::bicgstab(&jac, &rhs, None, &precond, &self.config.linear_settings)
                }
                LinearSolverKind::Gmres => {
                    krylov::gmres(&jac, &rhs, None, &precond, &self.config.linear_settings)
                }
            };
            // A non-converged inner linear solve is tolerated (inexact Newton);
            // only a non-finite direction is fatal.
            if !dx.iter().all(|v| v.is_finite()) {
                return Err(PflotranError::Convergence(format!(
                    "Newton step contains a non-finite value at iteration {iter} \
                     (linear solve returned {} components, residual norm {norm})",
                    dx.len()
                )));
            }

            // (op-v6s.4 step 4) backtracking / Armijo line search.
            let lambda = self.line_search(sys, x, &dx, norm, &mut x_trial, &mut f_trial)?;

            // Update x ← x + λ·dx.
            for i in 0..n {
                x[i] += lambda * dx[i];
            }
        }

        let last = history.last().copied().unwrap_or(f64::NAN);
        Err(PflotranError::Convergence(format!(
            "Newton did not converge in {} iterations (final ||F||₂ = {last:e}, \
             abs_tol {:e}, rel_tol {:e})",
            self.config.max_iterations, self.config.abs_tol, self.config.rel_tol
        )))
    }

    /// Backtracking line search returning the accepted step scale `λ ∈ (0, 1]`.
    ///
    /// Tries `λ = 1, 1/2, 1/4, …` up to [`NewtonConfig::max_backtracks`]
    /// halvings, accepting the first `λ` whose trial residual satisfies the
    /// Armijo sufficient-decrease test `||F(x + λ·dx)||₂ ≤ (1 − c·λ)·||F(x)||₂`
    /// with `c = 1e-4`. If none passes, it falls back to the `λ` that produced
    /// the smallest finite trial norm, so the Newton step always makes some
    /// progress. With `max_backtracks == 0` this always returns `λ = 1`
    /// (pure full-step Newton). Returns [`PflotranError::Convergence`] only if
    /// every trial residual is non-finite.
    fn line_search<S: NonlinearSystem>(
        &self,
        sys: &mut S,
        x: &[f64],
        dx: &[f64],
        base_norm: f64,
        x_trial: &mut [f64],
        f_trial: &mut [f64],
    ) -> Result<f64, PflotranError> {
        const ARMIJO_C: f64 = 1e-4;
        let n = x.len();

        let mut lambda = 1.0_f64;
        let mut best_lambda = 1.0_f64;
        let mut best_norm = f64::INFINITY;

        for _ in 0..=self.config.max_backtracks {
            for i in 0..n {
                x_trial[i] = x[i] + lambda * dx[i];
            }
            sys.assemble_residual(x_trial, f_trial)?;
            let trial_norm = nrm2(f_trial);

            if trial_norm.is_finite() && trial_norm < best_norm {
                best_norm = trial_norm;
                best_lambda = lambda;
            }
            if trial_norm.is_finite() && trial_norm <= (1.0 - ARMIJO_C * lambda) * base_norm {
                return Ok(lambda);
            }
            lambda *= 0.5;
        }

        if best_norm.is_finite() {
            Ok(best_lambda)
        } else {
            Err(PflotranError::Convergence(
                "line search found no finite residual along the Newton direction".to_string(),
            ))
        }
    }
}

/// Euclidean (2-)norm of a vector, `sqrt(Σ vᵢ²)`.
///
/// Inlined here rather than pulling a BLAS dependency, keeping the crate pure
/// Rust and Android-clean.
fn nrm2(v: &[f64]) -> f64 {
    v.iter().map(|&x| x * x).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1D Bratu problem `-u'' = λ·exp(u)` on `[0,1]` with `u(0)=u(1)=0`,
    /// discretized on `n` interior nodes with spacing `h = 1/(n+1)`.
    ///
    /// Residual: `F_i = -(u_{i-1} - 2u_i + u_{i+1})/h² - λ·exp(u_i)`
    /// (with `u_{-1} = u_n = 0`). Jacobian is tridiagonal:
    /// `∂F_i/∂u_i = 2/h² - λ·exp(u_i)`, `∂F_i/∂u_{i±1} = -1/h²`.
    /// Line-graph addressing: `owner[f] = f`, `neighbour[f] = f+1`.
    struct Bratu1D {
        n: usize,
        h2: f64,
        lambda: f64,
    }

    impl Bratu1D {
        fn new(n: usize, lambda: f64) -> Self {
            let h = 1.0 / (n as f64 + 1.0);
            Bratu1D {
                n,
                h2: h * h,
                lambda,
            }
        }
    }

    impl NonlinearSystem for Bratu1D {
        fn n_dof(&self) -> usize {
            self.n
        }
        fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
            let owner = (0..self.n - 1).collect();
            let neighbour = (1..self.n).collect();
            (owner, neighbour)
        }
        fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
            let inv = 1.0 / self.h2;
            for i in 0..self.n {
                let left = if i == 0 { 0.0 } else { x[i - 1] };
                let right = if i == self.n - 1 { 0.0 } else { x[i + 1] };
                out[i] = -(left - 2.0 * x[i] + right) * inv - self.lambda * x[i].exp();
            }
            Ok(())
        }
        fn assemble_jacobian(&mut self, x: &[f64], jac: &mut LduMatrix) -> Result<(), PflotranError> {
            let inv = 1.0 / self.h2;
            for d in jac.diag.iter_mut() {
                *d = 0.0;
            }
            for u in jac.upper.iter_mut() {
                *u = 0.0;
            }
            for l in jac.lower.iter_mut() {
                *l = 0.0;
            }
            for i in 0..self.n {
                jac.diag[i] = 2.0 * inv - self.lambda * x[i].exp();
            }
            // One face per off-diagonal; both super- and sub-diagonal are -1/h².
            for f in 0..jac.n_internal_faces {
                jac.upper[f] = -inv;
                jac.lower[f] = -inv;
            }
            Ok(())
        }
    }

    /// Constant linear system `F(x) = A·x - b` with `A` tridiagonal.
    /// Newton's Jacobian is the constant `A`, so it converges in one step to
    /// `A⁻¹ b`.
    struct LinearSys {
        diag: Vec<f64>,
        off: Vec<f64>, // super/sub-diagonal (symmetric), len n-1
        b: Vec<f64>,
    }

    impl NonlinearSystem for LinearSys {
        fn n_dof(&self) -> usize {
            self.diag.len()
        }
        fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
            let n = self.diag.len();
            ((0..n - 1).collect(), (1..n).collect())
        }
        fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
            let n = self.diag.len();
            for i in 0..n {
                let mut v = self.diag[i] * x[i];
                if i > 0 {
                    v += self.off[i - 1] * x[i - 1];
                }
                if i < n - 1 {
                    v += self.off[i] * x[i + 1];
                }
                out[i] = v - self.b[i];
            }
            Ok(())
        }
        fn assemble_jacobian(&mut self, _x: &[f64], jac: &mut LduMatrix) -> Result<(), PflotranError> {
            for (i, d) in jac.diag.iter_mut().enumerate() {
                *d = self.diag[i];
            }
            for f in 0..jac.n_internal_faces {
                jac.upper[f] = self.off[f];
                jac.lower[f] = self.off[f];
            }
            Ok(())
        }
    }

    #[test]
    fn bratu_converges_and_residual_decreases() {
        let mut sys = Bratu1D::new(15, 1.0);
        let solver = NewtonSolver::new(NewtonConfig {
            max_iterations: 50,
            abs_tol: 1e-10,
            rel_tol: 1e-12,
            linear: LinearSolverKind::BiCGStab,
            preconditioner: PreconditionerKind::Ilu0,
            linear_settings: KrylovSettings {
                tolerance: 1e-12,
                max_iter: 500,
                restart: 30,
            },
            max_backtracks: 10,
        });
        let mut x = vec![0.0; sys.n_dof()];
        let report = solver.solve(&mut sys, &mut x).expect("Bratu should converge");

        assert!(report.converged);
        assert!(
            report.final_residual_norm < 1e-10,
            "final residual {} not below tol",
            report.final_residual_norm
        );
        // Newton residual history must be (weakly) monotone non-increasing.
        for w in report.residual_history.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-14,
                "residual increased: {} -> {}",
                w[0],
                w[1]
            );
        }
        // The Bratu solution is symmetric and strictly positive in the interior.
        for &ui in &x {
            assert!(ui > 0.0, "interior solution should be positive, got {ui}");
        }
        // Symmetry about the mid-node.
        let n = x.len();
        for i in 0..n / 2 {
            assert!(
                (x[i] - x[n - 1 - i]).abs() < 1e-6,
                "solution not symmetric at {i}"
            );
        }
    }

    #[test]
    fn linear_system_converges_in_one_step() {
        // A = [[2,-1],[-1,2]], b = [1,1]  =>  x = [1,1].
        let mut sys = LinearSys {
            diag: vec![2.0, 2.0],
            off: vec![-1.0],
            b: vec![1.0, 1.0],
        };
        let solver = NewtonSolver::new(NewtonConfig {
            max_iterations: 20,
            abs_tol: 1e-12,
            rel_tol: 1e-14,
            linear: LinearSolverKind::BiCGStab,
            preconditioner: PreconditionerKind::Jacobi,
            linear_settings: KrylovSettings {
                tolerance: 1e-14,
                max_iter: 200,
                restart: 30,
            },
            max_backtracks: 0, // full Newton — a linear system needs no damping
        });
        let mut x = vec![0.0, 0.0];
        let report = solver.solve(&mut sys, &mut x).expect("linear solve converges");

        assert!(report.converged);
        // One Newton step lands on the root; the convergence test then fires at
        // iteration 1.
        assert_eq!(report.iterations, 1);
        assert!((x[0] - 1.0).abs() < 1e-9, "x0 = {}", x[0]);
        assert!((x[1] - 1.0).abs() < 1e-9, "x1 = {}", x[1]);
    }

    #[test]
    fn zero_iteration_budget_reports_nonconvergence() {
        let mut sys = LinearSys {
            diag: vec![2.0, 2.0],
            off: vec![-1.0],
            b: vec![1.0, 1.0],
        };
        let solver = NewtonSolver::new(NewtonConfig {
            max_iterations: 0, // no steps allowed; initial residual is nonzero
            ..NewtonConfig::default()
        });
        let mut x = vec![0.0, 0.0];
        let result = solver.solve(&mut sys, &mut x);
        assert!(
            matches!(result, Err(PflotranError::Convergence(_))),
            "expected Convergence error, got {result:?}"
        );
    }

    #[test]
    fn wrong_length_guess_is_rejected() {
        let mut sys = LinearSys {
            diag: vec![2.0, 2.0],
            off: vec![-1.0],
            b: vec![1.0, 1.0],
        };
        let solver = NewtonSolver::new(NewtonConfig::default());
        let mut x = vec![0.0; 3]; // wrong length (system has 2 DOF)
        let result = solver.solve(&mut sys, &mut x);
        assert!(matches!(result, Err(PflotranError::InvalidInput(_))));
    }
}
