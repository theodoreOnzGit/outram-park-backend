// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/accelerationSchemes/`:
//   accelerationScheme.{C,H}                 -> AccelerationScheme::None
//   accelerationSchemes.{C,H}                -> the registered instantiations
//   andersonMixing/andersonMixingScheme.{C,H} -> AndersonMixing
//   andersonMixing/andersonMixingSchemes.{C,H}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Anderson mixing — making a slowly-converging fixed-point iteration converge
//! fast.
//!
//! # The problem this solves
//!
//! Fuel performance is a strongly coupled loop: power sets temperature,
//! temperature sets thermal expansion and creep, deformation closes the
//! fuel/cladding gap, gap closure changes the gap conductance, which changes
//! temperature again. Solvers close that loop by **fixed-point iteration** —
//! guess a state, run one pass of every physics, take the answer as the next
//! guess, repeat until it stops changing:
//!
//! ```text
//! x_{k+1} = g(x_k)
//! ```
//!
//! This is *Picard iteration*, and it works, but its error decays by a constant
//! factor each pass. When the coupling is strong that factor is close to one,
//! and the loop takes hundreds or thousands of passes — each of which is a full
//! multiphysics solve.
//!
//! # What Anderson mixing does about it
//!
//! Instead of taking the newest iterate, Anderson mixing keeps the last few
//! iterates and forms the linear combination of them that best cancels the
//! *differences* between successive iterates. Where Picard uses one point of
//! history, Anderson uses `order + 1`, so it can see the shape of the
//! convergence and jump ahead along it. On a linear problem it is equivalent to
//! a Krylov method and can annihilate one error mode per unit of history depth
//! rather than damping every mode by the same factor.
//!
//! The method is also known as **Pulay mixing** or **DIIS** (direct inversion
//! in the iterative subspace), and upstream's implementation is the restarted
//! variant: it accumulates `order + 1` snapshots, extrapolates, throws the
//! history away, and starts collecting again.
//!
//! # It is not specific to corrosion
//!
//! Upstream keeps this in its own top-level `accelerationSchemes/` directory
//! and applies it wherever an outer iteration is slow. It lives inside
//! [`crate::corrosion`] in this port only because that is where the ported
//! directory landed; nothing about it is chemical. Anything of the shape
//! `x = g(x)` on a vector of `f64` can use it.
//!
//! # The algorithm, exactly
//!
//! With `order = m`, having collected snapshots `x_0 … x_m`:
//!
//! 1. Difference vectors `e_i = x_{i+1} − x_i` for `i = 0 … m−1`. These are the
//!    per-step corrections, and driving them to zero is the same as converging.
//! 2. Gram matrix `T_ij = <e_i, e_j>` (`m × m`, symmetric).
//! 3. Normalise: `T /= max|T_ij|`, then add `diagonal_factor` to the diagonal
//!    to make it diagonally dominant. Without that regularisation `T` is
//!    typically near-singular, because successive corrections are nearly
//!    parallel — which is precisely the situation Anderson mixing exists to
//!    exploit.
//! 4. Solve `T b = 1` (a vector of ones) and rescale so `Σ b_i = 1`. This is
//!    the DIIS condition: minimise `‖Σ b_i e_i‖` subject to the coefficients
//!    summing to one.
//! 5. New iterate: `x = Σ b_i · ((1 − α)·x_i + α·x_{i+1})`.
//! 6. Discard the history and start again.
//!
//! # Convergence guarantees — there are none
//!
//! Anderson mixing accelerates a convergent iteration; it does not make a
//! divergent one converge, and on a strongly nonlinear problem it can
//! occasionally take a worse step than plain Picard would have. Upstream's
//! `diagonal_factor` exists to blunt that. The one case with a firm theoretical
//! footing is a **linear** fixed point, where the method is a Krylov method and
//! the speed-up is real and measurable — which is why the tests below use a
//! linear problem with a closed-form solution as their reference.
//!
//! # Units
//!
//! None. This is a pure numerical utility on `f64` vectors; the caller's
//! vector may hold whatever it likes, in whatever units, as long as the
//! components are commensurate enough for a Euclidean inner product over them
//! to mean something. (Upstream has the same caveat, and the same silence about
//! it: mixing a temperature field and a stress field in one vector makes the
//! Gram matrix dominated by whichever has the larger numbers.)

// NaN-safe guards. Throughout this module a rejection test is written
// `!(x > 0.0)` rather than `x <= 0.0`, deliberately: the negated form is TRUE
// for NaN, so one comparison rejects negatives, zero and NaN together. Clippy's
// `neg_cmp_op_on_partial_ord` suggests the positive form, which would let a NaN
// through and propagate it into a physical result. The idiom is intentional.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::error::{OffbeatError, Result};

/// Default regularisation added to the diagonal of the projection matrix —
/// upstream's `diagonalFactor`, default `1e-4`.
pub const DEFAULT_DIAGONAL_FACTOR: f64 = 1.0e-4;

/// Default blending factor between old and new snapshots — upstream's `alpha`,
/// default `1.0` (use the newer iterate of each pair).
pub const DEFAULT_ALPHA: f64 = 1.0;

/// What a call to [`AccelerationScheme::accelerate`] did.
///
/// Returned rather than a bare `bool` because "the history is not full yet" and
/// "the history is full but the extrapolation was degenerate" are different
/// events with different consequences, and a caller that conflates them will
/// silently run an unaccelerated solve believing it is accelerated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelerationOutcome {
    /// The iterate was stored as a snapshot and left unchanged. The history is
    /// not yet full, or the scheme is [`AccelerationScheme::None`].
    Stored,

    /// The history was full; the iterate has been **overwritten** with the
    /// extrapolated value and the history reset.
    Accelerated,

    /// The history was full but no useful extrapolation exists — the
    /// difference vectors are all zero (the iteration has already converged
    /// exactly), or the projection matrix is singular, or the coefficients sum
    /// to zero. The iterate is left **unchanged** and the history is reset.
    ///
    /// This is not an error. It is the correct response to "there is nothing
    /// left to extrapolate", and the commonest cause is a converged iteration.
    /// Upstream instead aborts with a fatal error when the coefficients sum to
    /// zero, and produces `NaN` when the difference vectors vanish; both are
    /// reported here instead.
    Degenerate,
}

/// The outcome of running a fixed-point iteration to convergence.
///
/// Returned by [`AccelerationScheme::iterate`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedPointReport {
    /// Number of times `g` was evaluated. Each is one full pass of whatever
    /// the caller's iteration does, so this is the number to compare between
    /// schemes.
    pub iterations: usize,

    /// Final residual `‖g(x) − x‖₂` — the Euclidean norm of the last Picard
    /// correction, **before** any acceleration was applied to it.
    pub residual: f64,

    /// Whether [`residual`](Self::residual) reached the requested tolerance
    /// within the iteration budget.
    pub converged: bool,

    /// How many times the scheme actually extrapolated, i.e. returned
    /// [`AccelerationOutcome::Accelerated`]. Zero for
    /// [`AccelerationScheme::None`].
    pub accelerations: usize,
}

/// Restarted Anderson mixing (Pulay / DIIS) over `f64` vectors — upstream
/// `andersonMixingScheme`, `TypeName("andersonMixing")`.
///
/// See the [module documentation](self) for the algorithm and for what it does
/// and does not guarantee.
///
/// # Owning the history
///
/// The struct owns its snapshots by value in a `Vec<Vec<f64>>`. No `Box`, no
/// `dyn`, no lifetime parameters — per the workspace `CLAUDE.md` Rust design
/// rules. Memory is `(order + 1) × n` doubles, so a depth-5 scheme over a
/// million-cell field costs 48 MB; that is the real cost of the method and it
/// is why `order` is usually 3–8 rather than 50.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::corrosion::{AccelerationScheme, AndersonMixing};
///
/// // x = g(x) with g(x) = 0.99*x + 1, whose fixed point is x = 100.
/// let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(3));
/// let mut x = vec![0.0];
/// let report = scheme.iterate(&mut x, 1.0e-12, 5000, |current, next| {
///     next[0] = 0.99 * current[0] + 1.0;
/// });
///
/// assert!(report.converged);
/// assert!((x[0] - 100.0).abs() < 1.0e-8);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AndersonMixing {
    /// Acceleration order — the number of snapshots kept, minus one.
    order: usize,
    /// Blending factor between the older and newer snapshot of each pair.
    alpha: f64,
    /// Regularisation added to the diagonal of the normalised projection
    /// matrix.
    diagonal_factor: f64,
    /// Snapshots collected so far, oldest first; at most `order + 1` of them.
    snapshots: Vec<Vec<f64>>,
}

impl AndersonMixing {
    /// Anderson mixing of the given `order` with upstream's default `alpha`
    /// (1.0) and `diagonal_factor` (1e-4).
    ///
    /// # Choosing `order`
    ///
    /// `order` is the number of past corrections the extrapolation can see. It
    /// must be at least 1 — an order of 0 has no difference vectors and nothing
    /// to extrapolate from — and **is clamped up to 1** if a smaller value is
    /// passed, rather than silently producing a scheme that never accelerates.
    ///
    /// Larger is not automatically better: history costs memory, the projection
    /// matrix becomes more ill-conditioned as the older corrections lose
    /// relevance, and because this is the *restarted* variant, a larger order
    /// also means more plain Picard steps between extrapolations. Upstream's
    /// own usage example uses 5. On a linear problem the useful ceiling is the
    /// number of distinct error modes.
    #[must_use]
    pub fn new(order: usize) -> Self {
        Self::with_parameters(order, DEFAULT_ALPHA, DEFAULT_DIAGONAL_FACTOR)
    }

    /// Anderson mixing with every parameter given explicitly.
    ///
    /// - `order` — history depth; clamped up to at least 1. See
    ///   [`new`](Self::new).
    /// - `alpha` — blending factor \[-\] in `[0, 1]`. The extrapolation uses
    ///   `(1 − α)·x_i + α·x_{i+1}` for each snapshot pair, so `α = 1`
    ///   (upstream's default) uses the newer iterate of each pair and `α = 0`
    ///   the older. Values below 1 damp the step, which can stabilise a
    ///   badly-behaved nonlinear iteration at the cost of speed. Non-finite
    ///   values fall back to 1.0.
    /// - `diagonal_factor` — regularisation added to the diagonal of the
    ///   projection matrix **after** it is normalised to a maximum entry of 1,
    ///   so it is a relative quantity. Upstream's default is `1e-4`. Larger
    ///   values pull the extrapolation back towards plain Picard; smaller
    ///   values sharpen it but risk a near-singular solve. Negative or
    ///   non-finite values fall back to 1e-4.
    #[must_use]
    pub fn with_parameters(order: usize, alpha: f64, diagonal_factor: f64) -> Self {
        Self {
            order: order.max(1),
            alpha: if alpha.is_finite() {
                alpha
            } else {
                DEFAULT_ALPHA
            },
            diagonal_factor: if diagonal_factor.is_finite() && diagonal_factor >= 0.0 {
                diagonal_factor
            } else {
                DEFAULT_DIAGONAL_FACTOR
            },
            snapshots: Vec::new(),
        }
    }

    /// The acceleration order \[-\] — the history depth minus one.
    #[must_use]
    pub fn order(&self) -> usize {
        self.order
    }

    /// The blending factor `α` \[-\].
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// The projection-matrix diagonal regularisation \[-\].
    #[must_use]
    pub fn diagonal_factor(&self) -> f64 {
        self.diagonal_factor
    }

    /// How many snapshots are currently held, from 0 to `order + 1`.
    #[must_use]
    pub fn stored_snapshots(&self) -> usize {
        self.snapshots.len()
    }

    /// Discard the history.
    ///
    /// Call this whenever the iteration's meaning changes — a new timestep, a
    /// mesh change, a change of boundary conditions — because extrapolating
    /// across such a change mixes snapshots of two different problems.
    /// [`iterate`](AccelerationScheme::iterate) does it automatically at the
    /// start of each call.
    pub fn reset(&mut self) {
        self.snapshots.clear();
    }
}

/// Which acceleration scheme an outer iteration uses.
///
/// One variant per scheme in upstream OFFBEAT's `accelerationScheme` run-time
/// selection table. Dispatch is by `match`, never by a trait object, per the
/// workspace `CLAUDE.md` "No trait objects" rule.
#[derive(Debug, Clone, PartialEq)]
pub enum AccelerationScheme {
    /// No acceleration — plain Picard iteration. Upstream's base
    /// `accelerationScheme`, `TypeName("none")`.
    ///
    /// [`accelerate`](Self::accelerate) always returns
    /// [`AccelerationOutcome::Stored`] and leaves the iterate alone, so this is
    /// the honest baseline to compare an accelerated run against — and the
    /// thing to fall back to when acceleration misbehaves.
    None,

    /// Restarted Anderson mixing. Upstream `andersonMixingScheme`,
    /// `TypeName("andersonMixing")`.
    Anderson(AndersonMixing),
}

impl AccelerationScheme {
    /// Discard any history the scheme holds. No-op for [`None`](Self::None).
    pub fn reset(&mut self) {
        match self {
            Self::None => {}
            Self::Anderson(scheme) => scheme.reset(),
        }
    }

    /// The minimum number of iterations before this scheme can extrapolate at
    /// all — upstream's `minIter()`.
    ///
    /// `1` for [`None`](Self::None), `order + 1` for Anderson mixing. A caller
    /// whose convergence test might stop the loop early should honour this, or
    /// the acceleration will never fire.
    #[must_use]
    pub fn min_iterations(&self) -> usize {
        match self {
            Self::None => 1,
            Self::Anderson(scheme) => scheme.order + 1,
        }
    }

    /// Offer the current iterate to the scheme, and let it extrapolate if it
    /// can.
    ///
    /// This is upstream's `accelerate()`: it stores `x` as a snapshot, and once
    /// `order + 1` snapshots have accumulated it **overwrites `x` in place**
    /// with the extrapolated iterate and clears the history.
    ///
    /// The return value says which of those happened — see
    /// [`AccelerationOutcome`]. `x` is modified **only** on
    /// [`Accelerated`](AccelerationOutcome::Accelerated).
    ///
    /// # Vector length
    ///
    /// Every snapshot in one cycle must have the same length. If `x`'s length
    /// changes mid-cycle the history is discarded and `x` starts a fresh cycle,
    /// which is the only safe response — the difference vectors of two
    /// different-sized states are meaningless.
    ///
    /// # Example — the cadence
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{
    ///     AccelerationOutcome, AccelerationScheme, AndersonMixing,
    /// };
    ///
    /// let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(2));
    /// assert_eq!(scheme.min_iterations(), 3);
    ///
    /// let mut x = vec![1.0, 2.0];
    /// // Two stores, then an extrapolation on the third.
    /// assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Stored);
    /// x[0] += 0.5;
    /// assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Stored);
    /// x[0] += 0.25;
    /// assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Accelerated);
    /// ```
    pub fn accelerate(&mut self, x: &mut [f64]) -> AccelerationOutcome {
        match self {
            Self::None => AccelerationOutcome::Stored,
            Self::Anderson(scheme) => anderson_accelerate(scheme, x),
        }
    }

    /// Run `x ← g(x)` to convergence, accelerating with this scheme.
    ///
    /// The history is reset on entry, so each call is an independent solve.
    ///
    /// # Parameters
    ///
    /// - `x` — the iterate, updated in place. Its initial value is the starting
    ///   guess; on return it holds the converged (or best-effort) answer.
    /// - `tolerance` — convergence threshold on `‖g(x) − x‖₂`, the Euclidean
    ///   norm of the Picard correction. This is an **absolute** tolerance, so
    ///   scale it to the magnitude of `x`.
    /// - `max_iterations` — hard cap on evaluations of `g`.
    /// - `g` — the fixed-point map. Called as `g(current, next)` and must
    ///   write `g(current)` into `next`; both slices have the length of `x`.
    ///   A closure, taken generically — **not** a trait object, per the
    ///   workspace "no `Box<dyn>`" rule — so it may capture whatever state it
    ///   needs by value or by reference at the call site.
    ///
    /// # Convergence is tested on the un-accelerated correction
    ///
    /// The residual is measured on the plain Picard step `g(x) − x` *before*
    /// the extrapolation is applied. That is deliberate: it is the honest
    /// residual of the problem being solved, and it cannot be made to look
    /// small by an extrapolation that happens to move a long way.
    ///
    /// # Example — comparing against no acceleration
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{AccelerationScheme, AndersonMixing};
    ///
    /// // A deliberately slow linear map: g(x) = 0.999*x + 1, fixed point 1000.
    /// let map = |current: &[f64], next: &mut [f64]| next[0] = 0.999 * current[0] + 1.0;
    ///
    /// let mut plain = vec![0.0];
    /// let picard = AccelerationScheme::None.iterate(&mut plain, 1.0e-9, 100_000, map);
    ///
    /// let mut fast = vec![0.0];
    /// let anderson = AccelerationScheme::Anderson(AndersonMixing::new(3))
    ///     .iterate(&mut fast, 1.0e-9, 100_000, map);
    ///
    /// assert!(picard.converged && anderson.converged);
    /// assert!((plain[0] - 1000.0).abs() < 1.0e-5);
    /// assert!((fast[0] - 1000.0).abs() < 1.0e-5);
    ///
    /// // Measured 2026-07-29: 20714 evaluations for Picard against 3641 for
    /// // Anderson at order 3 — a 5.7x speed-up to the same answer.
    /// assert!(anderson.iterations * 4 < picard.iterations);
    /// ```
    pub fn iterate<G>(
        &mut self,
        x: &mut [f64],
        tolerance: f64,
        max_iterations: usize,
        mut g: G,
    ) -> FixedPointReport
    where
        G: FnMut(&[f64], &mut [f64]),
    {
        self.reset();
        let mut next = vec![0.0; x.len()];
        let mut report = FixedPointReport {
            iterations: 0,
            residual: f64::INFINITY,
            converged: false,
            accelerations: 0,
        };

        while report.iterations < max_iterations {
            report.iterations += 1;
            g(x, &mut next);
            report.residual = euclidean_distance(&next, x);
            x.copy_from_slice(&next);

            if report.residual <= tolerance {
                report.converged = true;
                break;
            }
            if self.accelerate(x) == AccelerationOutcome::Accelerated {
                report.accelerations += 1;
            }
        }
        report
    }

    /// [`iterate`](Self::iterate), but reporting non-convergence as an error.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::MechanicsNotConverged`] if the residual has not reached
    /// `tolerance` within `max_iterations`. The variant is named for the
    /// mechanics solve because that is the crate's canonical non-convergence,
    /// but it carries exactly the three numbers wanted here: final residual,
    /// target, and iterations spent.
    ///
    /// [`OffbeatError::MechanicsNotConverged`]: crate::error::OffbeatError::MechanicsNotConverged
    pub fn iterate_checked<G>(
        &mut self,
        x: &mut [f64],
        tolerance: f64,
        max_iterations: usize,
        g: G,
    ) -> Result<FixedPointReport>
    where
        G: FnMut(&[f64], &mut [f64]),
    {
        let report = self.iterate(x, tolerance, max_iterations, g);
        if report.converged {
            Ok(report)
        } else {
            Err(OffbeatError::MechanicsNotConverged {
                residual: report.residual,
                tolerance,
                iterations: report.iterations,
            })
        }
    }
}

/// One call of upstream's `andersonMixingScheme::accelerate()`.
///
/// Stores `x`, and on the `order + 1`-th call replaces `x` with the DIIS
/// extrapolation of the stored snapshots and clears the history. See the
/// [module documentation](self) for the algebra.
fn anderson_accelerate(scheme: &mut AndersonMixing, x: &mut [f64]) -> AccelerationOutcome {
    // A change of vector length invalidates every stored difference vector.
    if scheme
        .snapshots
        .first()
        .is_some_and(|first| first.len() != x.len())
    {
        scheme.snapshots.clear();
    }

    scheme.snapshots.push(x.to_vec());
    if scheme.snapshots.len() <= scheme.order {
        return AccelerationOutcome::Stored;
    }

    let m = scheme.order;

    // Step 1: difference vectors e_i = x_{i+1} - x_i.
    let mut differences: Vec<Vec<f64>> = Vec::with_capacity(m);
    for i in 0..m {
        let older = &scheme.snapshots[i];
        let newer = &scheme.snapshots[i + 1];
        differences.push(newer.iter().zip(older).map(|(a, b)| a - b).collect());
    }

    // Step 2: the symmetric Gram matrix, row-major m x m.
    let mut projection = vec![0.0; m * m];
    let mut largest = 0.0_f64;
    for i in 0..m {
        for j in 0..=i {
            let value = dot(&differences[i], &differences[j]);
            projection[i * m + j] = value;
            projection[j * m + i] = value;
            largest = largest.max(value.abs());
        }
    }

    // Every difference vector is zero: the iteration has already converged
    // exactly and there is nothing to extrapolate. Upstream divides by `Tmax`
    // here and produces NaN; this port reports it instead.
    if !(largest > 0.0) || !largest.is_finite() {
        scheme.snapshots.clear();
        return AccelerationOutcome::Degenerate;
    }

    // Step 3: normalise, then regularise the diagonal.
    for entry in &mut projection {
        *entry /= largest;
    }
    for i in 0..m {
        projection[i * m + i] += scheme.diagonal_factor;
    }

    // Step 4: solve T b = 1, then rescale so the coefficients sum to one.
    let mut coefficients = vec![1.0; m];
    if !lu_solve(&mut projection, &mut coefficients, m) {
        scheme.snapshots.clear();
        return AccelerationOutcome::Degenerate;
    }
    let sum: f64 = coefficients.iter().sum();
    if !sum.is_finite() || sum.abs() < 1.0e-300 {
        scheme.snapshots.clear();
        return AccelerationOutcome::Degenerate;
    }
    for coefficient in &mut coefficients {
        *coefficient /= sum;
    }
    if coefficients.iter().any(|c| !c.is_finite()) {
        scheme.snapshots.clear();
        return AccelerationOutcome::Degenerate;
    }

    // Step 5: reconstruct.
    let alpha = scheme.alpha;
    for value in x.iter_mut() {
        *value = 0.0;
    }
    for (i, &coefficient) in coefficients.iter().enumerate() {
        let older = &scheme.snapshots[i];
        let newer = &scheme.snapshots[i + 1];
        for ((value, &old), &new) in x.iter_mut().zip(older).zip(newer) {
            *value += coefficient * ((1.0 - alpha) * old + alpha * new);
        }
    }

    // Step 6: restart.
    scheme.snapshots.clear();
    AccelerationOutcome::Accelerated
}

/// Euclidean inner product of two equal-length vectors.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Euclidean distance `‖a − b‖₂`.
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

/// Solve the dense `n × n` system `A x = b` in place by LU decomposition with
/// partial pivoting — upstream calls OpenFOAM's `LUsolve`.
///
/// `matrix` is row-major and is destroyed. `rhs` holds `b` on entry and `x` on
/// return. Returns `false` if the matrix is singular to working precision, in
/// which case `rhs` is left in an unspecified state.
///
/// `n` here is the acceleration order — 3 to 8 in practice — so a plain dense
/// solve is the right tool and no external linear-algebra dependency is needed
/// (which also keeps this crate Android/Termux-buildable, per the workspace
/// portability rule).
fn lu_solve(matrix: &mut [f64], rhs: &mut [f64], n: usize) -> bool {
    debug_assert_eq!(matrix.len(), n * n);
    debug_assert_eq!(rhs.len(), n);

    for column in 0..n {
        // Partial pivot: the largest magnitude at or below the diagonal.
        let mut pivot_row = column;
        let mut pivot_magnitude = matrix[column * n + column].abs();
        for row in (column + 1)..n {
            let candidate = matrix[row * n + column].abs();
            if candidate > pivot_magnitude {
                pivot_magnitude = candidate;
                pivot_row = row;
            }
        }
        if !(pivot_magnitude > 0.0) || !pivot_magnitude.is_finite() {
            return false;
        }
        if pivot_row != column {
            for k in 0..n {
                matrix.swap(column * n + k, pivot_row * n + k);
            }
            rhs.swap(column, pivot_row);
        }

        let pivot = matrix[column * n + column];
        for row in (column + 1)..n {
            let factor = matrix[row * n + column] / pivot;
            if !factor.is_finite() {
                return false;
            }
            matrix[row * n + column] = 0.0;
            for k in (column + 1)..n {
                matrix[row * n + k] -= factor * matrix[column * n + k];
            }
            rhs[row] -= factor * rhs[column];
        }
    }

    // Back substitution.
    for row in (0..n).rev() {
        let mut value = rhs[row];
        for k in (row + 1)..n {
            value -= matrix[row * n + k] * rhs[k];
        }
        let pivot = matrix[row * n + row];
        if !(pivot.abs() > 0.0) {
            return false;
        }
        rhs[row] = value / pivot;
    }
    rhs.iter().all(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A linear fixed-point map `x_i ← g_i·x_i + c_i` with **distinct** decay
    /// factors, whose fixed point is known in closed form.
    ///
    /// This is the reference problem for the convergence tests. It is diagonal,
    /// so its exact solution `x_i* = c_i / (1 − g_i)` is analytic — no linear
    /// solve is needed to produce the reference, and no reference value is
    /// fabricated. Its spectrum is spread over `[0.059, 0.99]`, so plain Picard
    /// is dominated by the slowest mode while a Krylov-like method can
    /// annihilate modes one at a time.
    struct SpreadSpectrum {
        /// Per-component decay factors `g_i`, all in `(0, 1)`.
        factors: Vec<f64>,
        /// Per-component offsets `c_i`.
        offsets: Vec<f64>,
    }

    impl SpreadSpectrum {
        /// `n` components with `g_i` stepping down from `0.99` in steps of
        /// `0.049`, and `c_i = 1`. For `n = 20` the spectrum spans
        /// `[0.059, 0.99]`.
        fn new(n: usize) -> Self {
            let factors: Vec<f64> = (0..n).map(|i| 0.99 - 0.049 * (i as f64)).collect();
            assert!(
                factors.iter().all(|g| *g > 0.0 && *g < 1.0),
                "the test problem must be a contraction"
            );
            Self {
                offsets: vec![1.0; n],
                factors,
            }
        }

        /// The exact fixed point, analytically: `x_i* = c_i / (1 − g_i)`.
        fn exact(&self) -> Vec<f64> {
            self.factors
                .iter()
                .zip(&self.offsets)
                .map(|(g, c)| c / (1.0 - g))
                .collect()
        }

        /// Spectral radius, i.e. Picard's per-iteration error-reduction factor.
        fn spectral_radius(&self) -> f64 {
            self.factors.iter().cloned().fold(0.0, f64::max)
        }

        /// Apply the map.
        fn apply(&self, current: &[f64], next: &mut [f64]) {
            for i in 0..current.len() {
                next[i] = self.factors[i] * current[i] + self.offsets[i];
            }
        }
    }

    fn max_abs_difference(a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    }

    /// **REFERENCE-CHECKED against an exact analytic solution.** This is the
    /// one test in the corrosion module whose reference value is known in
    /// closed form rather than merely self-consistent, and it establishes both
    /// that Anderson mixing converges to the right answer and that it converges
    /// **substantially faster than plain Picard**.
    ///
    /// # Methodology
    ///
    /// - **Problem.** The 20-component linear fixed point
    ///   `x_i ← g_i·x_i + 1` with `g_i` stepping linearly from `0.99` down to
    ///   `0.059`. Diagonal, so the fixed point is exactly
    ///   `x_i* = 1/(1 − g_i)`, ranging from `100.0` to `1.0627` — an analytic
    ///   reference, not a fitted or previously-computed number.
    /// - **Schemes compared.** [`AccelerationScheme::None`] (plain Picard) and
    ///   [`AccelerationScheme::Anderson`] at orders 3, 5 and 8.
    /// - **Starting guess.** All zeros, for every scheme.
    /// - **Convergence criterion.** `‖g(x) − x‖₂ ≤ 1e-10`, measured on the
    ///   un-accelerated Picard correction, with a budget of 100 000
    ///   evaluations of `g`.
    /// - **Pass criteria.** (a) every scheme reaches the exact solution to
    ///   within `1e-6` absolute in every component; (b) every Anderson order
    ///   needs **fewer than half** of Picard's evaluations of `g`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// | Scheme | evaluations of `g` | speed-up over Picard | max error vs exact |
    /// |---|---|---|---|
    /// | Picard (`None`) | 2293 | 1.0× | 9.81e-09 |
    /// | Anderson, order 3 | 609 | 3.8× | 9.33e-09 |
    /// | Anderson, order 5 | 487 | 4.7× | 7.88e-09 |
    /// | Anderson, order 8 | 487 | 4.7× | 9.09e-09 |
    ///
    /// The spectral radius is `0.99`, so Picard's error falls by 1% per
    /// iteration; the observed 2293 iterations agree with the predicted
    /// `ln(tol·‖·‖ / err_0)/ln(0.99)` to within the noise of the stopping test.
    ///
    /// # Interpretation
    ///
    /// The speed-up is real and substantial — between four and five evaluations
    /// of the multiphysics saved for every one spent — and all four schemes
    /// land on the same analytic answer, so the acceleration is not trading
    /// accuracy for speed.
    ///
    /// It is, however, **much smaller than an unregularised DIIS would give**,
    /// and it stops improving past order 5. Both are consequences of upstream's
    /// default `diagonalFactor = 1e-4`, which is large relative to a projection
    /// matrix normalised to a maximum entry of one; it damps the extrapolation
    /// back towards plain Picard. The
    /// [companion test](Self::regularisation_costs_a_factor_of_three_in_speed_up)
    /// measures that cost directly. This test deliberately uses upstream's
    /// defaults, because reproducing OFFBEAT's behaviour is the point.
    ///
    /// None of this establishes anything about a **nonlinear** problem, where
    /// no such guarantee exists — see the [module documentation](super).
    #[test]
    fn anderson_beats_picard_on_a_linear_fixed_point_with_a_known_exact_solution() {
        let problem = SpreadSpectrum::new(20);
        let exact = problem.exact();

        // The analytic solution really is what it claims to be.
        assert!((problem.spectral_radius() - 0.99).abs() < 1.0e-12);
        assert!((exact[0] - 100.0).abs() < 1.0e-9);
        let mut check = vec![0.0; exact.len()];
        problem.apply(&exact, &mut check);
        assert!(
            max_abs_difference(&check, &exact) < 1.0e-12,
            "the analytic fixed point must be a fixed point"
        );

        let tolerance = 1.0e-10;
        let budget = 100_000;

        let mut x = vec![0.0; exact.len()];
        let picard =
            AccelerationScheme::None.iterate(&mut x, tolerance, budget, |c, n| problem.apply(c, n));
        assert!(picard.converged, "Picard must converge on a contraction");
        assert_eq!(picard.accelerations, 0);
        let picard_error = max_abs_difference(&x, &exact);
        assert!(
            picard_error < 1.0e-6,
            "Picard landed {picard_error} from the exact solution"
        );

        for order in [3, 5, 8] {
            let mut x = vec![0.0; exact.len()];
            let report = AccelerationScheme::Anderson(AndersonMixing::new(order)).iterate(
                &mut x,
                tolerance,
                budget,
                |c, n| problem.apply(c, n),
            );

            assert!(report.converged, "Anderson order {order} did not converge");
            assert!(
                report.accelerations > 0,
                "Anderson order {order} never actually extrapolated"
            );

            let error = max_abs_difference(&x, &exact);
            assert!(
                error < 1.0e-6,
                "Anderson order {order} landed {error} from the exact solution"
            );
            assert!(
                report.iterations * 2 < picard.iterations,
                "Anderson order {order} took {} iterations against Picard's {}",
                report.iterations,
                picard.iterations
            );
        }
    }

    /// **Reference-checked, in the same closed-form sense as the test above.**
    /// Upstream's default projection-matrix regularisation, `diagonalFactor =
    /// 1e-4`, is applied to a matrix that has already been normalised to a
    /// maximum entry of one, so it is a *relative* perturbation of 1e-4 on a
    /// matrix whose smallest eigenvalue is far smaller than that. It therefore
    /// does real damage to the extrapolation, and this test measures how much.
    ///
    /// # Methodology
    ///
    /// - Same 20-component analytic problem, same starting guess, same 1e-10
    ///   tolerance as
    ///   [the reference test](Self::anderson_beats_picard_on_a_linear_fixed_point_with_a_known_exact_solution).
    /// - Sweep `diagonal_factor` over `1e-4` (upstream's default), `1e-8` and
    ///   `0.0`, at orders 5 and 8.
    /// - Pass criterion: every configuration still converges to the exact
    ///   solution within `1e-6`, and reducing the regularisation
    ///   monotonically reduces the iteration count.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Evaluations of `g` (Picard's 2293 for reference):
    ///
    /// | `diagonal_factor` | order 5 | order 8 |
    /// |---|---|---|
    /// | `1e-4` (upstream default) | 487 | 487 |
    /// | `1e-8` | 253 | 208 |
    /// | `0.0` | 220 | 145 |
    ///
    /// So the speed-up over Picard rises from 4.7× to 10.4× (order 5) and from
    /// 4.7× to 15.8× (order 8) once the regularisation is removed.
    ///
    /// # Interpretation
    ///
    /// **Upstream's default costs roughly a factor of three in acceleration on
    /// this problem**, and it is what flattens the order-5-to-order-8 gain to
    /// nothing. That is not necessarily wrong: the regularisation is what stops
    /// a near-singular projection matrix producing a wild extrapolation on a
    /// hard nonlinear problem, and upstream's solvers are nonlinear. But it is
    /// a real cost, it is not documented upstream, and a user solving a
    /// well-conditioned problem should know they can turn it down with
    /// [`AndersonMixing::with_parameters`]. Recorded here rather than left for
    /// someone to rediscover.
    #[test]
    fn regularisation_costs_a_factor_of_three_in_speed_up() {
        let problem = SpreadSpectrum::new(20);
        let exact = problem.exact();
        let tolerance = 1.0e-10;
        let budget = 100_000;

        for order in [5, 8] {
            let mut previous = usize::MAX;
            for diagonal_factor in [1.0e-4, 1.0e-8, 0.0] {
                let mut x = vec![0.0; exact.len()];
                let report = AccelerationScheme::Anderson(AndersonMixing::with_parameters(
                    order,
                    DEFAULT_ALPHA,
                    diagonal_factor,
                ))
                .iterate(&mut x, tolerance, budget, |c, n| problem.apply(c, n));

                assert!(report.converged);
                assert!(max_abs_difference(&x, &exact) < 1.0e-6);
                assert!(
                    report.iterations <= previous,
                    "order {order}: relaxing the regularisation to {diagonal_factor:e} \
                     raised the count from {previous} to {}",
                    report.iterations
                );
                previous = report.iterations;
            }
        }

        // The recorded end points of the sweep.
        for (order, diagonal_factor, recorded) in [
            (5, 1.0e-4, 487.0),
            (5, 0.0, 220.0),
            (8, 1.0e-4, 487.0),
            (8, 0.0, 145.0),
        ] {
            let mut x = vec![0.0; exact.len()];
            let report = AccelerationScheme::Anderson(AndersonMixing::with_parameters(
                order,
                DEFAULT_ALPHA,
                diagonal_factor,
            ))
            .iterate(&mut x, tolerance, budget, |c, n| problem.apply(c, n));
            assert!(
                (report.iterations as f64 / recorded - 1.0).abs() < 0.25,
                "order {order}, diagonal_factor {diagonal_factor:e}: {} iterations, \
                 recorded {recorded}",
                report.iterations
            );
        }
    }

    /// Self-consistency check on the recorded iteration counts of the reference
    /// test above, kept separate so that a change in performance is reported as
    /// a distinct failure from a change in correctness.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Picard 2293 evaluations of `g`; Anderson 609 at order 3, 487 at order 5,
    /// 487 at order 8 — all with upstream's default regularisation. The
    /// tolerance below is deliberately loose (±25%) because these counts are
    /// floating-point-sensitive; a real regression changes them by an order of
    /// magnitude, not by a few percent.
    #[test]
    fn the_recorded_convergence_counts_have_not_drifted() {
        let problem = SpreadSpectrum::new(20);
        let tolerance = 1.0e-10;
        let budget = 100_000;

        let mut x = vec![0.0; 20];
        let picard =
            AccelerationScheme::None.iterate(&mut x, tolerance, budget, |c, n| problem.apply(c, n));
        assert!(
            (picard.iterations as f64 / 2293.0 - 1.0).abs() < 0.25,
            "Picard took {} iterations, recorded 2293",
            picard.iterations
        );

        for (order, recorded) in [(3, 609.0), (5, 487.0), (8, 487.0)] {
            let mut x = vec![0.0; 20];
            let report = AccelerationScheme::Anderson(AndersonMixing::new(order)).iterate(
                &mut x,
                tolerance,
                budget,
                |c, n| problem.apply(c, n),
            );
            assert!(
                (report.iterations as f64 / recorded - 1.0).abs() < 0.25,
                "Anderson order {order} took {} iterations, recorded {recorded}",
                report.iterations
            );
        }
    }

    /// **REFERENCE-CHECKED against an exact linear-system residual.** The
    /// previous test's problem is diagonal, so the components never talk to one
    /// another. This one couples them, and checks the answer against the
    /// residual of the linear system the fixed point solves — which is exact,
    /// computable in closed form, and does not require knowing the solution.
    ///
    /// # Methodology
    ///
    /// - **Problem.** `x ← G x + c` with `G` the 30×30 symmetric tridiagonal
    ///   Toeplitz matrix with `0.49` off the diagonal and zero on it, and
    ///   `c = 1`. Its eigenvalues are `0.98·cos(kπ/31)`, `k = 1…30`, so the
    ///   spectral radius is `0.98·cos(π/31) = 0.97497` and the spectrum is
    ///   spread symmetrically about zero — a harder problem for Picard than the
    ///   diagonal one, because the error has no single dominant sign.
    /// - **Reference.** The fixed point satisfies `(I − G) x = c` exactly, so
    ///   the reference is `‖(I − G) x − c‖∞ = 0`. Computed independently of the
    ///   iteration.
    /// - **Pass criterion.** Residual of the linear system below `1e-8`, and
    ///   Anderson using fewer than half of Picard's evaluations of `g`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// | Scheme | evaluations of `g` | `‖(I−G)x − c‖∞` |
    /// |---|---|---|
    /// | Picard | 974 | 2.41e-11 |
    /// | Anderson, order 4 | 291 | 2.27e-11 |
    /// | Anderson, order 6 | 224 | 2.57e-11 |
    /// | Anderson, order 8 | 225 | 2.75e-11 |
    ///
    /// Anderson at order 6 is a **4.35× speed-up** to the same residual, and
    /// its answer agrees with Picard's component-wise to `5.8e-11`.
    ///
    /// # Interpretation
    ///
    /// The speed-up is of the same size as on the diagonal problem and, as
    /// there, saturates by order 6 — the tridiagonal spectrum is dense and
    /// symmetric about zero, so there are no isolated modes for a higher-order
    /// extrapolation to pick off, and upstream's default regularisation caps
    /// the gain anyway. What this test adds over the diagonal one is that the
    /// method works on a **coupled** system, where the difference vectors are
    /// not simply rescaled copies of one another.
    #[test]
    fn anderson_converges_a_coupled_linear_system_to_its_exact_residual() {
        const N: usize = 30;
        const OFF: f64 = 0.49;

        let apply = |current: &[f64], next: &mut [f64]| {
            for i in 0..N {
                let mut value = 1.0;
                if i > 0 {
                    value += OFF * current[i - 1];
                }
                if i + 1 < N {
                    value += OFF * current[i + 1];
                }
                next[i] = value;
            }
        };

        // Residual of (I - G) x = c, in the infinity norm.
        let residual = |x: &[f64]| {
            let mut worst: f64 = 0.0;
            for i in 0..N {
                let mut value = x[i];
                if i > 0 {
                    value -= OFF * x[i - 1];
                }
                if i + 1 < N {
                    value -= OFF * x[i + 1];
                }
                worst = worst.max((value - 1.0).abs());
            }
            worst
        };

        let tolerance = 1.0e-10;
        let budget = 100_000;

        let mut plain = vec![0.0; N];
        let picard = AccelerationScheme::None.iterate(&mut plain, tolerance, budget, apply);
        assert!(picard.converged);
        let picard_residual = residual(&plain);
        assert!(
            picard_residual < 1.0e-8,
            "Picard residual {picard_residual:e}"
        );

        let mut fast = vec![0.0; N];
        let anderson = AccelerationScheme::Anderson(AndersonMixing::new(6))
            .iterate(&mut fast, tolerance, budget, apply);
        assert!(anderson.converged);
        let anderson_residual = residual(&fast);
        assert!(
            anderson_residual < 1.0e-8,
            "Anderson residual {anderson_residual:e}"
        );

        // The two schemes agree on the answer.
        assert!(max_abs_difference(&plain, &fast) < 1.0e-6);

        // ...and Anderson gets there in fewer than half the evaluations.
        assert!(
            anderson.iterations * 2 < picard.iterations,
            "Anderson took {} against Picard's {}",
            anderson.iterations,
            picard.iterations
        );
    }

    /// Self-consistency check on the store/extrapolate cadence: an order-`m`
    /// scheme must store `m` snapshots and extrapolate on the `(m+1)`-th call,
    /// then start over.
    #[test]
    fn the_acceleration_cadence_matches_the_order() {
        for order in 1..=6 {
            let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(order));
            assert_eq!(scheme.min_iterations(), order + 1);

            let mut x = vec![0.0; 4];
            // Two full cycles.
            for cycle in 0..2 {
                for step in 0..order {
                    for (k, value) in x.iter_mut().enumerate() {
                        *value = (cycle * 10 + step * 4 + k) as f64 * 0.5 + 1.0;
                    }
                    let before = x.clone();
                    assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Stored);
                    assert_eq!(x, before, "a Stored outcome must not modify the iterate");
                }
                for (k, value) in x.iter_mut().enumerate() {
                    *value = (cycle * 10 + order * 4 + k) as f64 * 0.5 + 1.0;
                }
                assert_eq!(
                    scheme.accelerate(&mut x),
                    AccelerationOutcome::Accelerated,
                    "order {order} should extrapolate on call {}",
                    order + 1
                );
                assert!(x.iter().all(|v| v.is_finite()));
            }
        }

        // The `None` scheme never accelerates and never touches the iterate.
        let mut scheme = AccelerationScheme::None;
        assert_eq!(scheme.min_iterations(), 1);
        let mut x = vec![1.0, 2.0, 3.0];
        for _ in 0..10 {
            assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Stored);
        }
        assert_eq!(x, vec![1.0, 2.0, 3.0]);
    }

    /// A converged iteration has zero difference vectors, so there is nothing
    /// to extrapolate. Upstream divides by a zero `Tmax` and produces `NaN`;
    /// this port reports [`AccelerationOutcome::Degenerate`] and leaves the
    /// iterate alone. **This guard is this port's, not upstream's.**
    #[test]
    fn an_already_converged_iteration_is_degenerate_not_nan() {
        let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(3));
        let mut x = vec![7.0, -2.0, 0.5];
        for _ in 0..3 {
            assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Stored);
        }
        assert_eq!(scheme.accelerate(&mut x), AccelerationOutcome::Degenerate);
        assert_eq!(x, vec![7.0, -2.0, 0.5], "the iterate must be untouched");
        assert!(x.iter().all(|v| v.is_finite()));

        // The history was reset, so the cadence starts again.
        if let AccelerationScheme::Anderson(inner) = &scheme {
            assert_eq!(inner.stored_snapshots(), 0);
        }

        // And an exactly-converged fixed point still terminates cleanly.
        let mut x = vec![3.0];
        let report = AccelerationScheme::Anderson(AndersonMixing::new(2)).iterate(
            &mut x,
            1.0e-12,
            100,
            |current, next| next[0] = current[0],
        );
        assert!(report.converged);
        assert_eq!(report.iterations, 1);
        assert_eq!(x, vec![3.0]);
    }

    /// Parameter handling: the order is clamped to at least 1, and non-finite
    /// or negative parameters fall back to upstream's defaults rather than
    /// poisoning the arithmetic.
    #[test]
    fn parameters_are_sanitised_and_reported() {
        let clamped = AndersonMixing::new(0);
        assert_eq!(clamped.order(), 1);
        assert_eq!(clamped.alpha(), DEFAULT_ALPHA);
        assert_eq!(clamped.diagonal_factor(), DEFAULT_DIAGONAL_FACTOR);
        assert_eq!(clamped.stored_snapshots(), 0);

        let sane = AndersonMixing::with_parameters(5, 0.7, 1.0e-6);
        assert_eq!(sane.order(), 5);
        assert_eq!(sane.alpha(), 0.7);
        assert_eq!(sane.diagonal_factor(), 1.0e-6);

        let rubbish = AndersonMixing::with_parameters(4, f64::NAN, -1.0);
        assert_eq!(rubbish.alpha(), DEFAULT_ALPHA);
        assert_eq!(rubbish.diagonal_factor(), DEFAULT_DIAGONAL_FACTOR);

        // A damped alpha still converges, just differently.
        let problem = SpreadSpectrum::new(20);
        let exact = problem.exact();
        let mut x = vec![0.0; 20];
        let report = AccelerationScheme::Anderson(AndersonMixing::with_parameters(
            5,
            0.5,
            DEFAULT_DIAGONAL_FACTOR,
        ))
        .iterate(&mut x, 1.0e-10, 100_000, |c, n| problem.apply(c, n));
        assert!(report.converged, "alpha = 0.5 must still converge");
        assert!(max_abs_difference(&x, &exact) < 1.0e-6);
    }

    /// `reset` discards the history, and a change of vector length is treated
    /// as a reset rather than as a size mismatch.
    #[test]
    fn reset_and_a_length_change_both_discard_the_history() {
        let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(3));
        let mut x = vec![1.0, 2.0];
        scheme.accelerate(&mut x);
        x[0] = 5.0;
        scheme.accelerate(&mut x);
        if let AccelerationScheme::Anderson(inner) = &scheme {
            assert_eq!(inner.stored_snapshots(), 2);
        }

        scheme.reset();
        if let AccelerationScheme::Anderson(inner) = &scheme {
            assert_eq!(inner.stored_snapshots(), 0);
        }

        // Refill partially, then change the length.
        scheme.accelerate(&mut x);
        scheme.accelerate(&mut x);
        let mut longer = vec![1.0, 2.0, 3.0];
        assert_eq!(scheme.accelerate(&mut longer), AccelerationOutcome::Stored);
        if let AccelerationScheme::Anderson(inner) = &scheme {
            assert_eq!(
                inner.stored_snapshots(),
                1,
                "the history should have restarted"
            );
        }

        // `None` resets harmlessly.
        let mut none = AccelerationScheme::None;
        none.reset();
        assert_eq!(none.min_iterations(), 1);
    }

    /// Self-consistency check on the dense solver the extrapolation rests on:
    /// it must reproduce a known solution, cope with a matrix needing a pivot
    /// swap, and refuse a singular one rather than returning nonsense.
    #[test]
    fn the_dense_lu_solver_is_correct_and_refuses_singular_systems() {
        // A 3x3 whose right-hand side was built by applying the matrix to
        // (1, 2, 3), so that vector is the exact solution by construction:
        //   2x +  y -  z =  1
        //  -3x -  y + 2z =  1
        //  -2x +  y + 2z =  6
        let mut matrix = vec![2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0];
        let mut rhs = vec![1.0, 1.0, 6.0];
        assert!(lu_solve(&mut matrix, &mut rhs, 3));
        for (got, want) in rhs.iter().zip([1.0, 2.0, 3.0]) {
            assert!((got - want).abs() < 1.0e-12, "{rhs:?}");
        }

        // A zero leading pivot forces a row swap.
        let mut matrix = vec![0.0, 1.0, 1.0, 0.0];
        let mut rhs = vec![2.0, 3.0];
        assert!(lu_solve(&mut matrix, &mut rhs, 2));
        assert!((rhs[0] - 3.0).abs() < 1.0e-12 && (rhs[1] - 2.0).abs() < 1.0e-12);

        // Identity.
        let mut matrix = vec![1.0, 0.0, 0.0, 1.0];
        let mut rhs = vec![4.0, -7.0];
        assert!(lu_solve(&mut matrix, &mut rhs, 2));
        assert_eq!(rhs, vec![4.0, -7.0]);

        // Singular: two identical rows.
        let mut matrix = vec![1.0, 2.0, 1.0, 2.0];
        let mut rhs = vec![3.0, 3.0];
        assert!(!lu_solve(&mut matrix, &mut rhs, 2));

        // All zeros.
        let mut matrix = vec![0.0; 4];
        let mut rhs = vec![1.0, 1.0];
        assert!(!lu_solve(&mut matrix, &mut rhs, 2));
    }

    /// `iterate_checked` reports a failure to converge instead of quietly
    /// returning a half-solved answer.
    #[test]
    fn iterate_checked_reports_non_convergence() {
        let problem = SpreadSpectrum::new(20);

        // A budget far too small.
        let mut x = vec![0.0; 20];
        let failure = AccelerationScheme::None
            .iterate_checked(&mut x, 1.0e-12, 5, |c, n| problem.apply(c, n));
        assert!(matches!(
            failure,
            Err(OffbeatError::MechanicsNotConverged { iterations: 5, .. })
        ));

        // A generous budget succeeds and agrees with the unchecked path.
        let mut x = vec![0.0; 20];
        let success = AccelerationScheme::Anderson(AndersonMixing::new(5))
            .iterate_checked(&mut x, 1.0e-10, 100_000, |c, n| problem.apply(c, n))
            .expect("should converge");
        assert!(success.converged);
        assert!(success.residual <= 1.0e-10);
    }
}
