//! Block multi-DOF Newton–Krylov solver layer — bead op-v6s.4.1.
//!
//! The scalar [`crate::solver`] handles one unknown per cell over
//! `outram-foam-basic-lib`'s scalar `LduMatrix`. Coupled multiphase physics
//! (e.g. the GENERAL mode: gas pressure + saturation + temperature) instead
//! carries `nb` unknowns per cell, with an `nb×nb` coupling block on every cell
//! and on every face. A scalar `LduMatrix` cannot represent that per-face
//! `nb×nb` coupling, so this module provides a self-contained block-sparse
//! layer built on the grid's face connectivity.
//!
//! # What lives here
//!
//! - [`BlockLduMatrix`] — a block-sparse matrix over owner/neighbour face
//!   addressing: an `nb×nb` diagonal block per cell and two `nb×nb` off-diagonal
//!   blocks per internal face (owner→neighbour in `upper`, neighbour→owner in
//!   `lower`).
//! - [`BlockJacobiPreconditioner`] — inverts each `nb×nb` diagonal block with a
//!   dense LU (`outram-foam-basic-lib`'s `SquareMatrix`) for use as a
//!   block-diagonal preconditioner.
//! - [`block_bicgstab`] — preconditioned BiCGStab on the flattened block system,
//!   using [`BlockLduMatrix::multiply`] as the sparse matrix–vector product.
//! - [`BlockNonlinearSystem`] — the compiler-contract for a block nonlinear
//!   system `F(x) = 0`, consumed via generics (static dispatch, never a trait
//!   object).
//! - [`BlockNewtonSolver`] — the Newton driver that mirrors the scalar
//!   [`crate::solver::NewtonSolver`] structure at the block level: assemble
//!   residual, converge-test (abs/rel), assemble block Jacobian, block-Jacobi
//!   preconditioned BiCGStab for `J·dx = -F`, Armijo backtracking line search,
//!   update.
//!
//! # State-vector layout
//!
//! All state vectors are length `n_cells * nb`. Cell `c`'s degrees of freedom
//! occupy the contiguous slice `[c*nb .. c*nb + nb]`. Blocks are stored
//! **row-major**: block entry `(i, j)` (row `i`, column `j`, both in `0..nb`) is
//! at offset `i*nb + j` within that block's `nb*nb`-length span.
//!
//! # Units
//!
//! As in the scalar layer, `x`, the residual, and the Jacobian entries are
//! dimensionless numeric coefficients — any `uom` unit bookkeeping belongs to
//! the flow mode that assembles them, not to this generic block layer.

use crate::error::PflotranError;
use outram_foam_basic_lib::krylov::vecops::{axpy, dot, nrm2};
use outram_foam_basic_lib::matrix::SquareMatrix;

/// Block-sparse matrix over grid face connectivity; `nb` unknowns per cell.
///
/// The sparsity mirrors an OpenFOAM-style LDU addressing but with dense `nb×nb`
/// blocks instead of scalar coefficients:
///
/// - one `nb×nb` **diagonal** block per cell — cell `c`'s block occupies
///   `diag[c*nb*nb .. (c+1)*nb*nb]`, row-major;
/// - two `nb×nb` **off-diagonal** blocks per internal face `f` — the
///   owner→neighbour coupling (`∂F_owner/∂x_neighbour`) in
///   `upper[f*nb*nb ..]`, and the neighbour→owner coupling
///   (`∂F_neighbour/∂x_owner`) in `lower[f*nb*nb ..]`.
///
/// Every face satisfies `owner[f] < neighbour[f]`. Within a block, entry
/// `(i, j)` sits at flat offset `i*nb + j`.
///
/// A state vector `x` acted on by this matrix has length `n_cells * nb`, with
/// cell `c`'s unknowns at `x[c*nb .. c*nb + nb]`.
pub struct BlockLduMatrix {
    /// Number of cells (equations groups); the state length is `n_cells * nb`.
    pub n_cells: usize,
    /// Unknowns per cell — the block dimension `nb`.
    pub nb: usize,
    /// Number of internal faces; the length of `owner` / `neighbour` and the
    /// count of `upper` / `lower` blocks.
    pub n_internal_faces: usize,
    /// Diagonal blocks, `n_cells` blocks of `nb*nb` row-major entries each.
    pub diag: Vec<f64>,
    /// Owner→neighbour off-diagonal blocks, `n_internal_faces` blocks of
    /// `nb*nb` entries each. `upper[f]` block holds `∂F_owner/∂x_neighbour`.
    pub upper: Vec<f64>,
    /// Neighbour→owner off-diagonal blocks, `n_internal_faces` blocks of
    /// `nb*nb` entries each. `lower[f]` block holds `∂F_neighbour/∂x_owner`.
    pub lower: Vec<f64>,
    /// Face owner cell indices, `owner[f] < neighbour[f]`.
    pub owner: Vec<usize>,
    /// Face neighbour cell indices.
    pub neighbour: Vec<usize>,
}

impl BlockLduMatrix {
    /// Allocate a zero-filled block matrix for `n_cells` cells, `nb` unknowns
    /// per cell, and the internal-face addressing `(owner, neighbour)`.
    ///
    /// `owner` and `neighbour` must have equal length (one entry per internal
    /// face); the number of internal faces is taken from that length. Each face
    /// is expected to satisfy `owner[f] < neighbour[f]`, matching the LDU
    /// addressing convention, and each index must be `< n_cells`.
    ///
    /// # Panics
    ///
    /// Panics if `owner.len() != neighbour.len()`.
    pub fn new(n_cells: usize, nb: usize, owner: Vec<usize>, neighbour: Vec<usize>) -> Self {
        assert_eq!(
            owner.len(),
            neighbour.len(),
            "owner and neighbour addressing arrays must have equal length"
        );
        let n_internal_faces = owner.len();
        let bb = nb * nb;
        BlockLduMatrix {
            n_cells,
            nb,
            n_internal_faces,
            diag: vec![0.0; n_cells * bb],
            upper: vec![0.0; n_internal_faces * bb],
            lower: vec![0.0; n_internal_faces * bb],
            owner,
            neighbour,
        }
    }

    /// Zero all coefficient blocks (`diag`, `upper`, `lower`) in place, keeping
    /// the addressing. Called at the start of each Jacobian assembly so the
    /// fixed sparsity is refilled rather than reallocated.
    pub fn zero(&mut self) {
        for v in self.diag.iter_mut() {
            *v = 0.0;
        }
        for v in self.upper.iter_mut() {
            *v = 0.0;
        }
        for v in self.lower.iter_mut() {
            *v = 0.0;
        }
    }

    /// Sparse block matrix–vector product `y = A·x`.
    ///
    /// Both `x` and the returned `y` have length `n_cells * nb`. The product is
    /// accumulated as, for each cell `c`, `y[c] += D_c · x[c]` (the diagonal
    /// block), and for each internal face `f` with owner `o` and neighbour `n`,
    /// `y[o] += U_f · x[n]` and `y[n] += L_f · x[o]` (each an `nb×nb` block
    /// times an `nb`-vector).
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != n_cells * nb`.
    pub fn multiply(&self, x: &[f64]) -> Vec<f64> {
        let nb = self.nb;
        assert_eq!(
            x.len(),
            self.n_cells * nb,
            "multiply: x length must be n_cells * nb"
        );
        let bb = nb * nb;
        let mut y = vec![0.0; self.n_cells * nb];

        // Diagonal blocks: y[c] += D_c · x[c].
        for c in 0..self.n_cells {
            let xc = &x[c * nb..c * nb + nb];
            let block = &self.diag[c * bb..c * bb + bb];
            let yc = &mut y[c * nb..c * nb + nb];
            block_gemv_add(block, nb, xc, yc);
        }

        // Off-diagonal face blocks.
        for f in 0..self.n_internal_faces {
            let o = self.owner[f];
            let n = self.neighbour[f];
            let ublock = &self.upper[f * bb..f * bb + bb];
            let lblock = &self.lower[f * bb..f * bb + bb];

            // y[o] += U_f · x[n]
            {
                let mut tmp = vec![0.0; nb];
                block_gemv_add(ublock, nb, &x[n * nb..n * nb + nb], &mut tmp);
                let yo = &mut y[o * nb..o * nb + nb];
                for i in 0..nb {
                    yo[i] += tmp[i];
                }
            }
            // y[n] += L_f · x[o]
            {
                let mut tmp = vec![0.0; nb];
                block_gemv_add(lblock, nb, &x[o * nb..o * nb + nb], &mut tmp);
                let yn = &mut y[n * nb..n * nb + nb];
                for i in 0..nb {
                    yn[i] += tmp[i];
                }
            }
        }

        y
    }

    /// Accumulate `v` into diagonal-block entry `(i, j)` of `cell` — i.e.
    /// `∂F_i/∂x_j` of that cell's local block. `i`, `j` are in `0..nb`.
    ///
    /// # Panics
    ///
    /// Panics if `cell >= n_cells` or `i`/`j` `>= nb`.
    pub fn add_diag(&mut self, cell: usize, i: usize, j: usize, v: f64) {
        let nb = self.nb;
        debug_assert!(cell < self.n_cells && i < nb && j < nb);
        self.diag[cell * nb * nb + i * nb + j] += v;
    }

    /// Accumulate `v` into owner→neighbour (`upper`) block entry `(i, j)` of
    /// `face` — the `∂F_owner,i/∂x_neighbour,j` coupling. `i`, `j` in `0..nb`.
    ///
    /// # Panics
    ///
    /// Panics if `face >= n_internal_faces` or `i`/`j` `>= nb`.
    pub fn add_upper(&mut self, face: usize, i: usize, j: usize, v: f64) {
        let nb = self.nb;
        debug_assert!(face < self.n_internal_faces && i < nb && j < nb);
        self.upper[face * nb * nb + i * nb + j] += v;
    }

    /// Accumulate `v` into neighbour→owner (`lower`) block entry `(i, j)` of
    /// `face` — the `∂F_neighbour,i/∂x_owner,j` coupling. `i`, `j` in `0..nb`.
    ///
    /// # Panics
    ///
    /// Panics if `face >= n_internal_faces` or `i`/`j` `>= nb`.
    pub fn add_lower(&mut self, face: usize, i: usize, j: usize, v: f64) {
        let nb = self.nb;
        debug_assert!(face < self.n_internal_faces && i < nb && j < nb);
        self.lower[face * nb * nb + i * nb + j] += v;
    }
}

/// Dense `nb×nb` (row-major) block times an `nb`-vector, accumulated into `out`:
/// `out += block · x`.
fn block_gemv_add(block: &[f64], nb: usize, x: &[f64], out: &mut [f64]) {
    for i in 0..nb {
        let row = &block[i * nb..i * nb + nb];
        let mut acc = 0.0;
        for j in 0..nb {
            acc += row[j] * x[j];
        }
        out[i] += acc;
    }
}

/// Block-Jacobi preconditioner: the inverse of each `nb×nb` diagonal block.
///
/// Applying it solves the block-diagonal system `D·z = r` cell-by-cell, using a
/// precomputed dense inverse of each diagonal block. A diagonal block that is
/// singular (its LU factorization fails) falls back to the identity block for
/// that cell, so `apply` never produces NaN from a failed factorization; the
/// preconditioner is then merely weaker there, not wrong.
pub struct BlockJacobiPreconditioner {
    /// Number of cells.
    n_cells: usize,
    /// Block dimension `nb`.
    nb: usize,
    /// Inverse of each diagonal block, `n_cells` blocks of `nb*nb` row-major
    /// entries. A singular block is stored as the `nb×nb` identity.
    inv_diag: Vec<f64>,
}

impl BlockJacobiPreconditioner {
    /// Factor each `nb×nb` diagonal block of `a` and store its inverse.
    ///
    /// Each inverse is formed column-by-column: the `k`-th column of the inverse
    /// is the LU solution of `D · col_k = e_k` (the `k`-th identity column),
    /// computed with `outram-foam-basic-lib`'s dense [`SquareMatrix::solve`]. If
    /// that solve fails (singular block), the identity is stored for that cell
    /// and preconditioning degrades gracefully rather than failing.
    pub fn new(a: &BlockLduMatrix) -> Self {
        let nb = a.nb;
        let bb = nb * nb;
        let mut inv_diag = vec![0.0; a.n_cells * bb];

        for c in 0..a.n_cells {
            let block = &a.diag[c * bb..c * bb + bb];

            // Load the diagonal block into a dense SquareMatrix (row-major).
            let mut m = SquareMatrix::new(nb);
            for i in 0..nb {
                for j in 0..nb {
                    m.set(i, j, block[i * nb + j]);
                }
            }

            let out = &mut inv_diag[c * bb..c * bb + bb];
            let mut singular = false;
            for k in 0..nb {
                // Identity column e_k.
                let mut e = vec![0.0; nb];
                e[k] = 1.0;
                match m.solve(&e) {
                    Ok(col) => {
                        // col is the k-th column of D^{-1}: inv[i][k] = col[i].
                        for i in 0..nb {
                            out[i * nb + k] = col[i];
                        }
                    }
                    Err(_) => {
                        singular = true;
                        break;
                    }
                }
            }
            if singular {
                // Fallback: identity block for this cell.
                for v in out.iter_mut() {
                    *v = 0.0;
                }
                for i in 0..nb {
                    out[i * nb + i] = 1.0;
                }
            }
        }

        BlockJacobiPreconditioner {
            n_cells: a.n_cells,
            nb,
            inv_diag,
        }
    }

    /// Apply the preconditioner: `z = M^{-1} r`, a block-diagonal solve.
    ///
    /// For each cell `c`, `z[c] = D_c^{-1} · r[c]` using the stored inverse
    /// block. Both `r` and `z` have length `n_cells * nb`.
    ///
    /// # Panics
    ///
    /// Panics if `r.len()` or `z.len()` differs from `n_cells * nb`.
    pub fn apply(&self, r: &[f64], z: &mut [f64]) {
        let nb = self.nb;
        assert_eq!(r.len(), self.n_cells * nb, "apply: r length mismatch");
        assert_eq!(z.len(), self.n_cells * nb, "apply: z length mismatch");
        let bb = nb * nb;
        for c in 0..self.n_cells {
            let block = &self.inv_diag[c * bb..c * bb + bb];
            let rc = &r[c * nb..c * nb + nb];
            let zc = &mut z[c * nb..c * nb + nb];
            for v in zc.iter_mut() {
                *v = 0.0;
            }
            block_gemv_add(block, nb, rc, zc);
        }
    }
}

/// Left-preconditioned BiCGStab on the flattened block system `A·x = b`.
///
/// Solves the block-sparse linear system with [`BlockLduMatrix::multiply`] as
/// the sparse matrix–vector product and the [`BlockJacobiPreconditioner`] as a
/// (right-applied here, via the standard `p̂ = M^{-1} p` / `ŝ = M^{-1} s`
/// scheme) preconditioner. Operates purely on flat `f64` slices using the
/// `outram-foam-basic-lib` BLAS-1 [`vecops`](outram_foam_basic_lib::krylov::vecops)
/// helpers.
///
/// # Arguments
///
/// - `a` — the block system matrix.
/// - `b` — right-hand side, length `n_cells * nb`.
/// - `x0` — optional initial guess (length `n_cells * nb`); `None` starts from
///   zero.
/// - `precond` — the block-Jacobi preconditioner (usually built from `a`).
/// - `tolerance` — relative-residual stopping tolerance `||b - A·x|| / ||b||`.
/// - `max_iter` — maximum BiCGStab iterations.
///
/// # Returns
///
/// `(solution, iterations, final_relative_residual, converged)`. The
/// `final_relative_residual` is recomputed as the *true* `||b - A·x|| / ||b||`
/// (not the recurrence residual). A zero right-hand side short-circuits to the
/// zero solution with `converged = true`. On a BiCGStab breakdown
/// (`rho ≈ 0` or `omega ≈ 0`) the iteration stops and returns the
/// best-residual iterate seen so far with `converged = false`.
pub fn block_bicgstab(
    a: &BlockLduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &BlockJacobiPreconditioner,
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f64>, usize, f64, bool) {
    let n = a.n_cells * a.nb;
    debug_assert_eq!(b.len(), n, "block_bicgstab: b length mismatch");

    let bnorm = nrm2(b);
    // Zero-RHS short-circuit: the solution is exactly zero.
    if bnorm == 0.0 {
        return (vec![0.0; n], 0, 0.0, true);
    }

    // x <- x0 (or zero).
    let mut x = match x0 {
        Some(g) => {
            debug_assert_eq!(g.len(), n, "block_bicgstab: x0 length mismatch");
            g.to_vec()
        }
        None => vec![0.0; n],
    };

    // r = b - A·x.
    let mut r = a.multiply(&x);
    for i in 0..n {
        r[i] = b[i] - r[i];
    }

    // Track the best iterate by true relative residual.
    let mut best_x = x.clone();
    let mut best_rel = nrm2(&r) / bnorm;
    if best_rel <= tolerance {
        return (x, 0, best_rel, true);
    }

    // Shadow residual r̂ = r0.
    let r_hat = r.clone();

    let mut rho: f64 = 1.0;
    let mut alpha: f64 = 1.0;
    let mut omega: f64 = 1.0;
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];

    // Preconditioner scratch.
    let mut p_hat = vec![0.0; n];
    let mut s_hat = vec![0.0; n];

    const BREAKDOWN: f64 = 1e-300;

    let mut converged = false;
    let mut iters = 0;

    for it in 1..=max_iter {
        iters = it;
        let rho_new = dot(&r_hat, &r);
        if rho_new.abs() < BREAKDOWN {
            // Breakdown: rho ~ 0.
            break;
        }
        let beta = (rho_new / rho) * (alpha / omega);
        // p = r + beta * (p - omega * v).
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }
        rho = rho_new;

        // p_hat = M^{-1} p.
        precond.apply(&p, &mut p_hat);
        // v = A · p_hat.
        v = a.multiply(&p_hat);

        let rhat_v = dot(&r_hat, &v);
        if rhat_v.abs() < BREAKDOWN {
            break;
        }
        alpha = rho / rhat_v;

        // s = r - alpha * v  (reuse r as s).
        let mut s = r.clone();
        axpy(-alpha, &v, &mut s);

        // Early convergence on the half-step: x += alpha * p_hat.
        let s_norm = nrm2(&s);
        if s_norm / bnorm <= tolerance {
            axpy(alpha, &p_hat, &mut x);
            // Confirm with the true residual before declaring victory; either
            // way this iterate is a candidate best and we stop here.
            let rel = true_rel_residual(a, b, &x, bnorm);
            if rel < best_rel {
                // best_rel is not read again before the break below; only the
                // best iterate matters here.
                best_x = x.clone();
            }
            if rel <= tolerance {
                converged = true;
            }
            break;
        }

        // s_hat = M^{-1} s.
        precond.apply(&s, &mut s_hat);
        // t = A · s_hat.
        let t = a.multiply(&s_hat);

        let tt = dot(&t, &t);
        if tt.abs() < BREAKDOWN {
            // t is zero: take the alpha step and stop.
            axpy(alpha, &p_hat, &mut x);
            break;
        }
        omega = dot(&t, &s) / tt;

        // x += alpha * p_hat + omega * s_hat.
        axpy(alpha, &p_hat, &mut x);
        axpy(omega, &s_hat, &mut x);

        // r = s - omega * t.
        r = s;
        axpy(-omega, &t, &mut r);

        let rel = nrm2(&r) / bnorm;
        if rel < best_rel {
            best_rel = rel;
            best_x = x.clone();
        }

        if rel <= tolerance {
            // Confirm with the true residual before declaring victory.
            let true_rel = true_rel_residual(a, b, &x, bnorm);
            if true_rel < best_rel {
                best_rel = true_rel;
                best_x = x.clone();
            }
            if true_rel <= tolerance {
                converged = true;
                break;
            }
        }

        if omega.abs() < BREAKDOWN {
            // Breakdown: omega ~ 0.
            break;
        }
    }

    // Report the best iterate with its recomputed true relative residual.
    let final_rel = true_rel_residual(a, b, &best_x, bnorm);
    (best_x, iters, final_rel, converged && final_rel <= tolerance)
}

/// True relative residual `||b - A·x|| / ||b||` (recomputed, not the recurrence
/// value). `bnorm` is `||b||` passed in to avoid recomputing it.
fn true_rel_residual(a: &BlockLduMatrix, b: &[f64], x: &[f64], bnorm: f64) -> f64 {
    let ax = a.multiply(x);
    let mut res = vec![0.0; b.len()];
    for i in 0..b.len() {
        res[i] = b[i] - ax[i];
    }
    nrm2(&res) / bnorm
}

/// Compiler-enforced contract for a block nonlinear system `F(x) = 0`.
///
/// Consumed via generics ([`BlockNewtonSolver::solve`] takes
/// `S: BlockNonlinearSystem`), so dispatch is **static** — never a trait object
/// (`Box<dyn _>` / `&dyn _`), per the workspace design rules.
///
/// The state vector `x` has length `n_cells * dof_per_cell`, with cell `c`'s
/// unknowns at `x[c*nb .. c*nb + nb]` (`nb = dof_per_cell`). The Jacobian
/// sparsity — the `(owner, neighbour)` face addressing — is **fixed for the
/// entire solve**; only the block coefficient values change per Newton
/// iteration, so the [`BlockLduMatrix`] is allocated once.
pub trait BlockNonlinearSystem {
    /// Number of cells; the state length is `n_cells() * dof_per_cell()`.
    fn n_cells(&self) -> usize;

    /// Unknowns per cell — the block dimension `nb`. Constant across the solve.
    fn dof_per_cell(&self) -> usize;

    /// The fixed internal-face addressing as `(owner, neighbour)` arrays, one
    /// entry per face with `owner[f] < neighbour[f]`. Called once per solve to
    /// size the [`BlockLduMatrix`].
    fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>);

    /// Assemble the residual `F(x)` into `out` (length
    /// `n_cells() * dof_per_cell()`). Newton drives this toward zero. Every
    /// entry of `out` must be written.
    fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError>;

    /// Assemble the block Jacobian `dF/dx` into `jac` (already sized to the
    /// addressing from [`ldu_addressing`](Self::ldu_addressing)). The
    /// implementation must [`zero`](BlockLduMatrix::zero) `jac` and then fill
    /// its diagonal / upper / lower blocks — e.g. with
    /// [`BlockLduMatrix::add_diag`] / [`add_upper`](BlockLduMatrix::add_upper) /
    /// [`add_lower`](BlockLduMatrix::add_lower).
    fn assemble_jacobian(
        &mut self,
        x: &[f64],
        jac: &mut BlockLduMatrix,
    ) -> Result<(), PflotranError>;
}

/// Configuration for the block Newton–Krylov solver.
///
/// Convergence is declared when the residual 2-norm falls below `abs_tol`, or
/// below `rel_tol` times the initial residual norm. See
/// [`BlockNewtonConfig::default`] for the shipped defaults.
pub struct BlockNewtonConfig {
    /// Maximum Newton iterations before giving up with
    /// [`PflotranError::Convergence`].
    pub max_iterations: usize,
    /// Absolute convergence tolerance: converged when `||F||₂ < abs_tol`.
    pub abs_tol: f64,
    /// Relative convergence tolerance: converged when
    /// `||F||₂ < rel_tol · ||F₀||₂`.
    pub rel_tol: f64,
    /// Relative-residual tolerance passed to the inner [`block_bicgstab`] solve.
    pub linear_tolerance: f64,
    /// Maximum iterations for the inner [`block_bicgstab`] solve.
    pub linear_max_iter: usize,
    /// Maximum backtracking line-search halvings per Newton step. `0` disables
    /// the line search (pure full-step Newton, `λ = 1`).
    pub max_backtracks: usize,
}

impl Default for BlockNewtonConfig {
    /// Defaults: 50 Newton iterations, `abs_tol = 1e-9`, `rel_tol = 1e-8`,
    /// `linear_tolerance = 1e-10`, `linear_max_iter = 2000`, and up to 10
    /// line-search backtracks.
    fn default() -> Self {
        BlockNewtonConfig {
            max_iterations: 50,
            abs_tol: 1e-9,
            rel_tol: 1e-8,
            linear_tolerance: 1e-10,
            linear_max_iter: 2000,
            max_backtracks: 10,
        }
    }
}

/// Outcome of a [`BlockNewtonSolver::solve`] call.
///
/// Returned by value on success; on failure the same information is summarized
/// inside the [`PflotranError::Convergence`] message.
#[derive(Debug, Clone)]
pub struct BlockNewtonReport {
    /// Number of Newton iterations performed (residual evaluations at which the
    /// convergence test was applied).
    pub iterations: usize,
    /// Whether the tolerance was met. Always `true` in a successfully returned
    /// report — a `false` outcome is surfaced as an `Err` instead.
    pub converged: bool,
    /// The residual 2-norm `||F||₂` at exit.
    pub final_residual_norm: f64,
    /// The residual 2-norm at each Newton iteration, in order.
    pub residual_history: Vec<f64>,
}

/// Serial pure-Rust block Newton–Krylov nonlinear solver.
///
/// Construct with [`BlockNewtonSolver::new`] and drive a
/// [`BlockNonlinearSystem`] to a root with [`BlockNewtonSolver::solve`]. The
/// driver mirrors the scalar [`crate::solver::NewtonSolver`] structure at the
/// block level: assemble `F`, converge-test (abs/rel), assemble the block
/// Jacobian, build a [`BlockJacobiPreconditioner`], solve `J·dx = -F` with
/// [`block_bicgstab`], then take a backtracking/Armijo line-search step.
pub struct BlockNewtonSolver {
    /// The Newton / line-search / linear-solver configuration.
    pub config: BlockNewtonConfig,
}

impl BlockNewtonSolver {
    /// Create a block Newton solver with the given configuration.
    pub fn new(config: BlockNewtonConfig) -> Self {
        BlockNewtonSolver { config }
    }

    /// Solve `F(x) = 0`, updating `x` in place with the converged root.
    ///
    /// On success returns a [`BlockNewtonReport`]. Returns
    /// [`PflotranError::Convergence`] if the tolerance is not met within
    /// [`BlockNewtonConfig::max_iterations`] or if a NaN/Inf appears in the
    /// residual or the Newton step, and [`PflotranError::InvalidInput`] if `x`
    /// has the wrong length or the addressing arrays are inconsistent.
    ///
    /// As in the scalar layer, an inner *linear* solve that does not fully
    /// converge is tolerated (inexact Newton); only a **non-finite** step
    /// direction is fatal.
    pub fn solve<S: BlockNonlinearSystem>(
        &self,
        sys: &mut S,
        x: &mut [f64],
    ) -> Result<BlockNewtonReport, PflotranError> {
        let n_cells = sys.n_cells();
        let nb = sys.dof_per_cell();
        let n = n_cells * nb;
        if x.len() != n {
            return Err(PflotranError::InvalidInput(format!(
                "initial guess has length {} but the system has {n_cells} cells × {nb} DOF = {n}",
                x.len()
            )));
        }

        let (owner, neighbour) = sys.ldu_addressing();
        if owner.len() != neighbour.len() {
            return Err(PflotranError::InvalidInput(format!(
                "owner (len {}) and neighbour (len {}) addressing arrays differ",
                owner.len(),
                neighbour.len()
            )));
        }
        let mut jac = BlockLduMatrix::new(n_cells, nb, owner, neighbour);

        // Scratch buffers reused across iterations.
        let mut f = vec![0.0_f64; n]; // residual F(x)
        let mut rhs = vec![0.0_f64; n]; // -F
        let mut x_trial = vec![0.0_f64; n];
        let mut f_trial = vec![0.0_f64; n];

        let mut history: Vec<f64> = Vec::with_capacity(self.config.max_iterations + 1);
        let mut norm0: Option<f64> = None;

        for iter in 0..=self.config.max_iterations {
            sys.assemble_residual(x, &mut f)?;
            let norm = nrm2(&f);
            if !norm.is_finite() {
                return Err(PflotranError::Convergence(format!(
                    "residual norm is non-finite ({norm}) at block-Newton iteration {iter}"
                )));
            }
            history.push(norm);
            let base = *norm0.get_or_insert(norm);

            if norm < self.config.abs_tol || norm < self.config.rel_tol * base {
                return Ok(BlockNewtonReport {
                    iterations: iter,
                    converged: true,
                    final_residual_norm: norm,
                    residual_history: history,
                });
            }

            if iter == self.config.max_iterations {
                break;
            }

            // Assemble block Jacobian, build block-Jacobi preconditioner, solve
            // J·dx = -F.
            sys.assemble_jacobian(x, &mut jac)?;
            for i in 0..n {
                rhs[i] = -f[i];
            }

            let precond = BlockJacobiPreconditioner::new(&jac);
            let (dx, _lin_iters, _lin_rel, _lin_conv) = block_bicgstab(
                &jac,
                &rhs,
                None,
                &precond,
                self.config.linear_tolerance,
                self.config.linear_max_iter,
            );

            if !dx.iter().all(|v| v.is_finite()) {
                return Err(PflotranError::Convergence(format!(
                    "block-Newton step contains a non-finite value at iteration {iter} \
                     (residual norm {norm})"
                )));
            }

            let lambda = self.line_search(sys, x, &dx, norm, &mut x_trial, &mut f_trial)?;
            for i in 0..n {
                x[i] += lambda * dx[i];
            }
        }

        let last = history.last().copied().unwrap_or(f64::NAN);
        Err(PflotranError::Convergence(format!(
            "block-Newton did not converge in {} iterations (final ||F||₂ = {last:e}, \
             abs_tol {:e}, rel_tol {:e})",
            self.config.max_iterations, self.config.abs_tol, self.config.rel_tol
        )))
    }

    /// Backtracking / Armijo line search returning the accepted step scale
    /// `λ ∈ (0, 1]`.
    ///
    /// Tries `λ = 1, 1/2, 1/4, …` up to
    /// [`BlockNewtonConfig::max_backtracks`] halvings, accepting the first `λ`
    /// whose trial residual satisfies `||F(x + λ·dx)||₂ ≤ (1 − c·λ)·||F(x)||₂`
    /// with `c = 1e-4`. If none passes, falls back to the `λ` with the smallest
    /// finite trial norm so a step is always taken. With `max_backtracks == 0`
    /// this always returns `λ = 1`. Returns [`PflotranError::Convergence`] only
    /// if every trial residual is non-finite.
    fn line_search<S: BlockNonlinearSystem>(
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
                "block line search found no finite residual along the Newton direction".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Materialize a [`BlockLduMatrix`] into a dense row-major `(n, n)` matrix,
    /// `n = n_cells * nb`, for cross-checking `multiply` and linear solves.
    fn to_dense(a: &BlockLduMatrix) -> (usize, Vec<f64>) {
        let nb = a.nb;
        let n = a.n_cells * nb;
        let mut d = vec![0.0; n * n];
        // Diagonal blocks.
        for c in 0..a.n_cells {
            for i in 0..nb {
                for j in 0..nb {
                    let gi = c * nb + i;
                    let gj = c * nb + j;
                    d[gi * n + gj] += a.diag[c * nb * nb + i * nb + j];
                }
            }
        }
        // Face blocks: upper = owner-row, neighbour-col; lower = neighbour-row,
        // owner-col.
        for f in 0..a.n_internal_faces {
            let o = a.owner[f];
            let nb_cell = a.neighbour[f];
            for i in 0..nb {
                for j in 0..nb {
                    let up = a.upper[f * nb * nb + i * nb + j];
                    let lo = a.lower[f * nb * nb + i * nb + j];
                    // upper: row block = owner, col block = neighbour.
                    d[(o * nb + i) * n + (nb_cell * nb + j)] += up;
                    // lower: row block = neighbour, col block = owner.
                    d[(nb_cell * nb + i) * n + (o * nb + j)] += lo;
                }
            }
        }
        (n, d)
    }

    /// Dense row-major matvec `y = D·x`.
    fn dense_matvec(n: usize, d: &[f64], x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; n];
        for i in 0..n {
            let mut acc = 0.0;
            for j in 0..n {
                acc += d[i * n + j] * x[j];
            }
            y[i] = acc;
        }
        y
    }

    /// Build a small nb=2, 3-cell line-graph block matrix with distinct,
    /// nonsymmetric block entries for the SpMV / solve tests.
    fn build_small_block() -> BlockLduMatrix {
        // Line graph 0-1-2: faces (0,1) and (1,2).
        let owner = vec![0, 1];
        let neighbour = vec![1, 2];
        let mut a = BlockLduMatrix::new(3, 2, owner, neighbour);

        // Diagonal blocks — diagonally dominant, nonsymmetric.
        // cell 0
        a.add_diag(0, 0, 0, 8.0);
        a.add_diag(0, 0, 1, 1.0);
        a.add_diag(0, 1, 0, -0.5);
        a.add_diag(0, 1, 1, 7.0);
        // cell 1
        a.add_diag(1, 0, 0, 9.0);
        a.add_diag(1, 0, 1, 0.3);
        a.add_diag(1, 1, 0, 0.2);
        a.add_diag(1, 1, 1, 8.5);
        // cell 2
        a.add_diag(2, 0, 0, 7.5);
        a.add_diag(2, 0, 1, -0.4);
        a.add_diag(2, 1, 0, 0.6);
        a.add_diag(2, 1, 1, 9.2);

        // Face 0 (0-1) blocks — nonsymmetric upper != lower.
        a.add_upper(0, 0, 0, -1.0);
        a.add_upper(0, 0, 1, 0.2);
        a.add_upper(0, 1, 0, 0.1);
        a.add_upper(0, 1, 1, -0.8);
        a.add_lower(0, 0, 0, -0.9);
        a.add_lower(0, 0, 1, 0.05);
        a.add_lower(0, 1, 0, -0.15);
        a.add_lower(0, 1, 1, -1.1);

        // Face 1 (1-2) blocks.
        a.add_upper(1, 0, 0, -0.7);
        a.add_upper(1, 0, 1, 0.1);
        a.add_upper(1, 1, 0, 0.25);
        a.add_upper(1, 1, 1, -0.6);
        a.add_lower(1, 0, 0, -0.65);
        a.add_lower(1, 0, 1, -0.2);
        a.add_lower(1, 1, 0, 0.3);
        a.add_lower(1, 1, 1, -0.95);

        a
    }

    #[test]
    fn block_spmv_matches_dense() {
        let a = build_small_block();
        let (n, dense) = to_dense(&a);
        assert_eq!(n, 6);

        // A couple of fixed (non-RNG) test vectors.
        let xs = [
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            vec![1.0, -2.0, 3.0, -4.0, 5.0, -6.0],
            vec![0.5, 1.5, -2.5, 3.5, -0.25, 0.75],
        ];
        for x in &xs {
            let y_block = a.multiply(x);
            let y_dense = dense_matvec(n, &dense, x);
            for i in 0..n {
                assert!(
                    (y_block[i] - y_dense[i]).abs() < 1e-12,
                    "SpMV mismatch at {i}: block {} vs dense {}",
                    y_block[i],
                    y_dense[i]
                );
            }
        }
    }

    #[test]
    fn block_bicgstab_matches_dense_lu() {
        let a = build_small_block();
        let (n, dense) = to_dense(&a);

        // A nonsymmetric RHS.
        let b = vec![1.0, -1.0, 2.0, 0.5, -3.0, 1.5];

        // Dense LU reference via SquareMatrix.
        let mut m = SquareMatrix::new(n);
        for i in 0..n {
            for j in 0..n {
                m.set(i, j, dense[i * n + j]);
            }
        }
        let x_ref = m.solve(&b).expect("dense block system is nonsingular");

        let precond = BlockJacobiPreconditioner::new(&a);
        let (x, iters, rel, converged) = block_bicgstab(&a, &b, None, &precond, 1e-12, 1000);
        assert!(converged, "BiCGStab did not converge (rel {rel}, iters {iters})");

        let mut max_err = 0.0f64;
        for i in 0..n {
            max_err = max_err.max((x[i] - x_ref[i]).abs());
        }
        assert!(
            max_err < 1e-6,
            "block BiCGStab solution differs from dense LU by {max_err} (rel resid {rel})"
        );
    }

    #[test]
    fn block_bicgstab_zero_rhs_short_circuits() {
        let a = build_small_block();
        let precond = BlockJacobiPreconditioner::new(&a);
        let b = vec![0.0; 6];
        let (x, iters, rel, converged) = block_bicgstab(&a, &b, None, &precond, 1e-12, 100);
        assert!(converged);
        assert_eq!(iters, 0);
        assert_eq!(rel, 0.0);
        assert!(x.iter().all(|&v| v == 0.0));
    }

    /// Two coupled 1D nonlinear diffusion–reaction equations on `n` cells with
    /// unknowns `(u, v)` per cell (nb = 2). Manufactured solution
    /// `u*(x) = sin(πx)`, `v*(x) = x(1−x)` on `[0,1]`, Dirichlet BCs baked into
    /// the residual via ghost values from the exact solution.
    ///
    /// Governing residuals per interior cell `c` (spacing `h`, `x_c` the cell
    /// centre):
    ///
    /// `F_u = -(u_{c-1} - 2u_c + u_{c+1})/h² + κ·u_c·v_c − s_u(x_c)`
    /// `F_v = -(v_{c-1} - 2v_c + v_{c+1})/h² + κ·u_c·v_c − s_v(x_c)`
    ///
    /// with `κ` the coupling strength and the manufactured sources `s_u`, `s_v`
    /// chosen so that `(u*, v*)` is the exact root. This gives a tridiagonal
    /// per-variable Laplacian plus a *local* `u–v` coupling in each diagonal
    /// block (off-diagonal block entries `∂F_u/∂v = κu_c`, `∂F_v/∂u = κv_c`).
    struct CoupledDiffusionReaction {
        n: usize,
        h: f64,
        kappa: f64,
        /// Cell-centre coordinates.
        xc: Vec<f64>,
    }

    impl CoupledDiffusionReaction {
        fn new(n: usize, kappa: f64) -> Self {
            // Cell centres on [0,1]: x_c = (c + 0.5)*h, h = 1/n.
            let h = 1.0 / n as f64;
            let xc = (0..n).map(|c| (c as f64 + 0.5) * h).collect();
            CoupledDiffusionReaction { n, h, kappa, xc }
        }

        fn u_exact(x: f64) -> f64 {
            (std::f64::consts::PI * x).sin()
        }
        fn v_exact(x: f64) -> f64 {
            x * (1.0 - x)
        }
        /// -u'' for u* = sin(πx) is π² sin(πx).
        fn u_lap(x: f64) -> f64 {
            let pi = std::f64::consts::PI;
            pi * pi * (pi * x).sin()
        }
        /// -v'' for v* = x(1−x) is +2.
        fn v_lap(_x: f64) -> f64 {
            2.0
        }
        /// Manufactured source s_u = -u''* + κ u* v*.
        fn s_u(&self, x: f64) -> f64 {
            Self::u_lap(x) + self.kappa * Self::u_exact(x) * Self::v_exact(x)
        }
        /// Manufactured source s_v = -v''* + κ u* v*.
        fn s_v(&self, x: f64) -> f64 {
            Self::v_lap(x) + self.kappa * Self::u_exact(x) * Self::v_exact(x)
        }
    }

    impl BlockNonlinearSystem for CoupledDiffusionReaction {
        fn n_cells(&self) -> usize {
            self.n
        }
        fn dof_per_cell(&self) -> usize {
            2
        }
        fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
            ((0..self.n - 1).collect(), (1..self.n).collect())
        }
        fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
            let inv = 1.0 / (self.h * self.h);
            let n = self.n;
            for c in 0..n {
                let u = x[c * 2];
                let v = x[c * 2 + 1];

                // Neighbour / ghost values (Dirichlet from exact solution at
                // domain ends; ghost cell-centre at x=-h/2 and x=1+h/2 mirror,
                // so use exact boundary value at 0 and 1 respectively).
                let (ul, vl) = if c == 0 {
                    // Ghost left: reflect exact boundary value at x=0.
                    // Use 2*bc - u_c approximation is unnecessary here; simplest
                    // consistent choice is a ghost centre value = exact(x_{-1}).
                    let xg = -0.5 * self.h;
                    (Self::u_exact(xg), Self::v_exact(xg))
                } else {
                    (x[(c - 1) * 2], x[(c - 1) * 2 + 1])
                };
                let (ur, vr) = if c == n - 1 {
                    let xg = 1.0 + 0.5 * self.h;
                    (Self::u_exact(xg), Self::v_exact(xg))
                } else {
                    (x[(c + 1) * 2], x[(c + 1) * 2 + 1])
                };

                let lap_u = -(ul - 2.0 * u + ur) * inv;
                let lap_v = -(vl - 2.0 * v + vr) * inv;

                out[c * 2] = lap_u + self.kappa * u * v - self.s_u(self.xc[c]);
                out[c * 2 + 1] = lap_v + self.kappa * u * v - self.s_v(self.xc[c]);
            }
            Ok(())
        }
        fn assemble_jacobian(
            &mut self,
            x: &[f64],
            jac: &mut BlockLduMatrix,
        ) -> Result<(), PflotranError> {
            jac.zero();
            let inv = 1.0 / (self.h * self.h);
            let n = self.n;
            for c in 0..n {
                let u = x[c * 2];
                let v = x[c * 2 + 1];
                // Diagonal block: Laplacian diagonal (2/h²) on each variable
                // plus local coupling from κ u v.
                // ∂F_u/∂u = 2/h² + κ v ; ∂F_u/∂v = κ u
                // ∂F_v/∂v = 2/h² + κ u ; ∂F_v/∂u = κ v
                jac.add_diag(c, 0, 0, 2.0 * inv + self.kappa * v);
                jac.add_diag(c, 0, 1, self.kappa * u);
                jac.add_diag(c, 1, 0, self.kappa * v);
                jac.add_diag(c, 1, 1, 2.0 * inv + self.kappa * u);
            }
            // Face blocks: -1/h² on each variable's own diagonal (no cross
            // coupling between variables across faces).
            for f in 0..jac.n_internal_faces {
                jac.add_upper(f, 0, 0, -inv);
                jac.add_upper(f, 1, 1, -inv);
                jac.add_lower(f, 0, 0, -inv);
                jac.add_lower(f, 1, 1, -inv);
            }
            Ok(())
        }
    }

    #[test]
    fn coupled_nonlinear_newton_converges_to_manufactured() {
        let n = 10;
        let kappa = 2.0;
        let mut sys = CoupledDiffusionReaction::new(n, kappa);
        let solver = BlockNewtonSolver::new(BlockNewtonConfig {
            max_iterations: 50,
            abs_tol: 1e-11,
            rel_tol: 1e-12,
            linear_tolerance: 1e-12,
            linear_max_iter: 2000,
            max_backtracks: 10,
        });

        // Start from zero.
        let mut xvec = vec![0.0; n * 2];
        let report = solver
            .solve(&mut sys, &mut xvec)
            .expect("coupled Newton should converge");

        assert!(report.converged);
        assert!(
            report.final_residual_norm < 1e-10,
            "final residual {} too large",
            report.final_residual_norm
        );
        // Residual history must be (weakly) monotone non-increasing.
        for w in report.residual_history.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-12,
                "residual increased: {} -> {}",
                w[0],
                w[1]
            );
        }

        // Compare against the manufactured solution at cell centres. The scheme
        // is 2nd-order, so on 10 cells the discretization error is O(h²) ~ 1e-2;
        // assert a generous but meaningful tolerance.
        let mut max_err = 0.0f64;
        for c in 0..n {
            let xc = sys.xc[c];
            let ue = CoupledDiffusionReaction::u_exact(xc);
            let ve = CoupledDiffusionReaction::v_exact(xc);
            max_err = max_err.max((xvec[c * 2] - ue).abs());
            max_err = max_err.max((xvec[c * 2 + 1] - ve).abs());
        }
        assert!(
            max_err < 5e-2,
            "manufactured-solution error {max_err} exceeds discretization tolerance"
        );
    }

    #[test]
    fn zero_iteration_budget_reports_nonconvergence() {
        let mut sys = CoupledDiffusionReaction::new(5, 1.0);
        let solver = BlockNewtonSolver::new(BlockNewtonConfig {
            max_iterations: 0,
            ..BlockNewtonConfig::default()
        });
        let mut xvec = vec![0.0; 5 * 2];
        let result = solver.solve(&mut sys, &mut xvec);
        assert!(
            matches!(result, Err(PflotranError::Convergence(_))),
            "expected Convergence error, got {result:?}"
        );
    }

    #[test]
    fn wrong_length_guess_is_rejected() {
        let mut sys = CoupledDiffusionReaction::new(5, 1.0);
        let solver = BlockNewtonSolver::new(BlockNewtonConfig::default());
        let mut xvec = vec![0.0; 7]; // wrong length (should be 10)
        let result = solver.solve(&mut sys, &mut xvec);
        assert!(matches!(result, Err(PflotranError::InvalidInput(_))));
    }
}
