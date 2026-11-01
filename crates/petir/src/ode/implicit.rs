// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PORTED, read 2026-11-01, from two upstreams:
//
// 1. The GNU Scientific Library (GSL) 2.8, commit
//    cf180cd7fbd06039a577f9c9ff0b428784765ac1:
//
//      ode-initval2/modnewton1.c  the modified Newton iteration
//                                 (`modnewton1_init`, `modnewton1_solve`)
//                                 Copyright (C) 2008, 2009, 2010 Tuomo Keskitalo
//      ode-initval2/rk1imp.c      the single implicit-Euler step of
//                                 `rk1imp_apply` (lines 291-326)
//                                 Copyright (C) 2009, 2010 Tuomo Keskitalo
//      ode-initval2/rksubs.c      the final substitution `y = y0 + h sum b fY`
//                                 Copyright (C) 2009, 2010 Tuomo Keskitalo
//      ode-initval2/cstd.c        `std_control_errlevel` (lines 146-166)
//                                 Copyright (C) 1996, 1997, 1998, 1999, 2000,
//                                 2007 Gerard Jungman, Brian Gough
//
//    GSL is GPL-3.0-or-later (verified from each per-file header).
//
// 2. OpenFOAM-dev, commit 198dd0e1083f89b1613636ddda82e5bf7eac6e69:
//
//      src/finiteVolume/finiteVolume/ddtSchemes/backwardDdtScheme/
//      backwardDdtScheme.C        the variable-step second-order backward
//                                 (BDF2) coefficients `coefft`, `coefft00`,
//                                 `coefft0` (lines 676-678 in `fvmDdt`) and
//                                 the Euler start `deltaT0_(vf)` (lines 59-69)
//                                 Copyright (C) 2011-2026 OpenFOAM Foundation
//
//    OpenFOAM is GPL-3.0-or-later (verified from the file header).
//
// This port is distributed under GPL-3.0-only.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.

//! Implicit integrators for stiff `dy/dt = f(t, y)`: backward Euler and BDF2
//! started with one backward-Euler step, each with a Newton inner solve
//! (gh:#1065, RELAP-7 Theory Manual §5.2.1–5.2.2, cited only).
//!
//! # What is here
//!
//! - [`backward_euler_step`] — one implicit Euler step.
//! - [`bdf2_step`] — one variable-step BDF2 step, given the state one step
//!   back and that step's size.
//! - [`solve_backward_euler`] and [`solve_bdf2`] — march over a caller-given
//!   time grid (uniform or not); `solve_bdf2` takes its first step with
//!   backward Euler.
//! - [`fd_jacobian`] — a forward-difference `df/dy`, for a caller without an
//!   analytic Jacobian.
//!
//! # Lineage — two ports, composed
//!
//! **The Newton solve is GSL's `modnewton1`** (`ode-initval2/modnewton1.c`), the
//! modified Newton iteration GSL's implicit Runge–Kutta steppers use, ported
//! for one stage: the iteration matrix `I - h a J` is formed once per step
//! from the Jacobian at the start of the step, factored by GSL's LU
//! ([`crate::linalg::lu_decomp`]), and reused; the iteration starts from the
//! old state; it stops on Hairer & Wanner's (§IV.8) contraction test
//! `eta_k ||dY / errlev||_2 < 1` with `eta_k = theta_k / (1 - theta_k)`, at
//! most **7** iterations, and declares divergence when `theta_k >= 1`. The
//! per-component error level is GSL's standard control
//! (`cstd.c:156`) with `a_y = 1`, `a_dydt = 0`:
//! `errlev_i = eps_rel |y_i| + eps_abs` ([`ImplicitTolerance`]).
//!
//! **Backward Euler is the single step of GSL's `rk1imp`**
//! (`rk1imp.c:291-326`): Newton for the stage `Y = y + h f(t + h, Y)`, then
//! `y_new = y + h f(t + h, Y)` through `rksubs`.
//!
//! **BDF2's coefficients are OpenFOAM's `backwardDdtScheme`**
//! (`backwardDdtScheme.C:676-678`): with `dt` the current and `dt0` the
//! previous step,
//!
//! ```text
//! coefft   = 1 + dt/(dt + dt0)
//! coefft00 = dt*dt/(dt0*(dt + dt0))
//! coefft0  = coefft + coefft00
//! (coefft*y_new - coefft0*y + coefft00*y_old)/dt = f(t + dt, y_new)
//! ```
//!
//! which at `dt == dt0` is `(3 y_new - 4 y + y_old)/(2 dt)`. GSL has no
//! fixed-order BDF2 to port: its `msbdf` is a variable-order (1–5) Nordsieck
//! method with its own step and order control, and cutting it down to order 2
//! on a caller's grid would no longer be `msbdf`. Rearranged, the BDF2 step is
//! `y_new = ybar + h (1/coefft) f(t + h, y_new)` with
//! `ybar = (coefft0 y - coefft00 y_old)/coefft`, which is exactly the
//! one-stage system `modnewton1` solves with `A = [[1/coefft]]`, `c = [1]` —
//! so the BDF2 step reuses the ported Newton unchanged.
//!
//! # Deviations, each stated
//!
//! - **DEVIATION — no step doubling.** `rk1imp_apply` takes the step, then
//!   two half steps, returns the half-step result and estimates the error
//!   from the difference (`rk1imp.c:328-400`). This module returns the
//!   single step (`y_onestep`) and no error estimate: the RELAP-7 scheme
//!   (§5.2.1) is plain backward Euler on a given step, and returning the
//!   two-half-step answer would make the observed order a property of
//!   Richardson-like doubling, not of backward Euler.
//! - **DEVIATION — exact Euler start.** OpenFOAM starts BDF2 by setting
//!   `dt0 = great = 1/DBL_EPSILON` when no old-old field exists
//!   (`backwardDdtScheme.C:62-64`; `great` from `doubleScalar.H`), which makes
//!   the first step Euler only to `O(dt/great)`. Here the first step of
//!   [`solve_bdf2`] is [`backward_euler_step`] exactly.
//! - **DEVIATION — two failure codes.** `modnewton1_solve` returns
//!   `GSL_FAILURE` both for divergence and for exhausting its 7 iterations,
//!   and GSL's driver then shrinks the step. Here they are
//!   [`PetirError::Diverged`] and [`PetirError::MaxIterations`], and **no
//!   step is retried**: the grid is the caller's.
//! - **Stage specialisation.** The Kronecker loops of `modnewton1_init` /
//!   `_solve` are written for `stage = 1`; the arithmetic of every
//!   expression is kept (`-h*a*J`, `1.0 - h*a*J`, `-1.0*Yk + y0 + h*sum`).
//!
//! # Units
//!
//! Bare `f64`, per the crate's units policy.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::{lu_decomp, lu_solve, nrm2, LuPermutation, Matrix};
use crate::multiroots::{fdjacobian, GSL_SQRT_DBL_EPSILON};
use crate::ode::{OdePoint, OdeSolution};
use crate::{PetirError, Result};

/// `GSL_DBL_EPSILON` (`gsl_machine.h:17`), spelled as upstream spells it.
#[allow(clippy::excessive_precision)]
const GSL_DBL_EPSILON: f64 = 2.2204460492503131e-16;
/// `GSL_DBL_MAX` (`gsl_machine.h:19`), `modnewton1`'s initial `eeta_prev`.
const GSL_DBL_MAX: f64 = 1.7976931348623157e+308;
/// `modnewton1_solve`'s iteration cap (`modnewton1.c:285`).
const MODNEWTON1_MAX_ITER: usize = 7;

/// The error level the Newton test is scaled by: GSL's standard control
/// (`cstd.c:156`) with `a_y = 1`, `a_dydt = 0` (`gsl_odeiv2_control_y_new`).
///
/// `errlev_i = eps_rel * |y_i| + eps_abs`, `y` the state at the start of the
/// step. Newton stops when the scaled correction is (predicted to be) below
/// one, so these set how well each implicit equation is solved — **not** the
/// truncation error, which the step size alone controls here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplicitTolerance {
    /// Absolute part, in the units of `y`.
    pub eps_abs: f64,
    /// Relative part, dimensionless.
    pub eps_rel: f64,
}

/// `std_control_errlevel` (`cstd.c:146-166`) for every component.
fn errlevel(tol: &ImplicitTolerance, y: &[f64], h: f64) -> Result<Vec<f64>> {
    let (a_y, a_dydt, dydt) = (1.0_f64, 0.0_f64, 0.0_f64);
    let mut out = Vec::with_capacity(y.len());
    for &yi in y {
        let e = tol.eps_rel * (a_y * yi.abs() + a_dydt * (h * dydt).abs()) + tol.eps_abs;
        // Upstream: GSL_ERROR ("errlev <= zero", GSL_ESANITY).
        if !(e > 0.0) {
            return Err(PetirError::Invalid);
        }
        out.push(e);
    }
    Ok(out)
}

/// `modnewton1_state_t` for one stage: the factored iteration matrix and the
/// contraction history.
struct ModNewton1 {
    /// LU factors of `I - h a J` (upstream `IhAJ`).
    ihaj: Matrix,
    /// Its row permutation (upstream `p`).
    p: LuPermutation,
    /// `eeta_prev`, reset to `GSL_DBL_MAX` by every `init`.
    eeta_prev: f64,
}

impl ModNewton1 {
    /// `modnewton1_init` (`modnewton1.c:169-219`) for `stage = 1`.
    fn init(a: f64, h: f64, dfdy: &Matrix) -> Result<Self> {
        let n = dfdy.rows();
        let mut ihaj = Matrix::zeros(n, n)?;
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    ihaj.set(i, j, -h * a * dfdy.get(i, j));
                } else {
                    ihaj.set(i, j, 1.0 - h * a * dfdy.get(i, j));
                }
            }
        }
        let p = lu_decomp(&mut ihaj)?;
        Ok(Self {
            ihaj,
            p,
            eeta_prev: GSL_DBL_MAX,
        })
    }

    /// `modnewton1_solve` (`modnewton1.c:225-419`) for `stage = 1`: solve
    /// `Y = y0 + h a f(t + c h, Y)` and return `Y` (upstream `YZ`).
    #[allow(clippy::too_many_arguments)]
    fn solve<F>(
        &mut self,
        f: &F,
        a: f64,
        c: f64,
        t: f64,
        h: f64,
        y0: &[f64],
        errlev: &[f64],
    ) -> Result<Vec<f64>>
    where
        F: Fn(f64, &[f64], &mut [f64]),
    {
        let dim = y0.len();
        let mut dyk = vec![0.0_f64; dim];
        let mut yk: Vec<f64> = y0.to_vec();
        let mut fyk = vec![0.0_f64; dim];
        let mut rhs = vec![0.0_f64; dim];
        let mut dscal = vec![0.0_f64; dim];

        let mut converged = false;
        let mut iter = 0usize;
        let mut dscal_norm = 0.0_f64;

        while !converged && iter < MODNEWTON1_MAX_ITER {
            iter += 1;

            // Update Y(k) and evaluate function.
            for (y, &d) in yk.iter_mut().zip(dyk.iter()) {
                *y += d;
            }
            f(t + c * h, &yk, &mut fyk);
            if fyk.iter().any(|v| !v.is_finite()) {
                return Err(PetirError::Domain);
            }

            // rhs = -Y(k) + y0 + h * sum_n A_jn f(Y_n)   (one stage)
            for (r, (&y, (&y0m, &fy))) in rhs
                .iter_mut()
                .zip(yk.iter().zip(y0.iter().zip(fyk.iter())))
            {
                let mut sum = 0.0_f64;
                sum += a * fy;
                // Upstream's `-1.0 * Yk[..]`, kept as written.
                #[allow(clippy::neg_multiply)]
                {
                    *r = -1.0 * y + y0m + h * sum;
                }
            }

            dyk = lu_solve(&self.ihaj, &self.p, &rhs)?;

            // Convergence (Hairer & Wanner IV.8), scaled by errlev.
            for (s, (&d, &e)) in dscal.iter_mut().zip(dyk.iter().zip(errlev.iter())) {
                *s = d / e;
            }
            let dscal_norm_prev = dscal_norm;
            dscal_norm = nrm2(&dscal);
            let theta_k = dscal_norm / dscal_norm_prev;

            let eeta_k = if iter > 1 {
                // An increasing step length indicates divergence.
                if theta_k >= 1.0 {
                    return Err(PetirError::Diverged);
                }
                theta_k / (1.0 - theta_k)
            } else {
                Real::powf(self.eeta_prev.max(GSL_DBL_EPSILON), 0.8)
            };
            self.eeta_prev = eeta_k;

            if eeta_k * dscal_norm < 1.0 {
                converged = true;
            }
        }

        if !converged {
            return Err(PetirError::MaxIterations);
        }

        // Give solution in YZ.
        Ok(yk.iter().zip(dyk.iter()).map(|(&y, &d)| y + d).collect())
    }
}

/// One-stage implicit step `Y = y0 + h a f(t + h, Y)`, `y_new = y0 + h b
/// f(t + h, Y)` with `b = a`: the shared body of `rk1imp`'s single step and
/// the BDF2 step. `y_err` is the state the error levels are taken from.
#[allow(clippy::too_many_arguments)]
fn one_stage_step<F, J>(
    f: &F,
    jac: &J,
    t: f64,
    h: f64,
    a: f64,
    y0: &[f64],
    y_err: &[f64],
    tol: &ImplicitTolerance,
) -> Result<Vec<f64>>
where
    F: Fn(f64, &[f64], &mut [f64]),
    J: Fn(f64, &[f64], &mut Matrix),
{
    let dim = y0.len();
    if dim == 0 || y_err.len() != dim || !h.is_finite() || h == 0.0 {
        return Err(PetirError::Invalid);
    }
    let c = 1.0_f64;
    let b = a;

    let errlev = errlevel(tol, y_err, h)?;

    // Evaluate the Jacobian at the start of the step (rk1imp.c:280).
    let mut dfdy = Matrix::zeros(dim, dim)?;
    jac(t, y_err, &mut dfdy);
    if dfdy.as_slice().iter().any(|v| !v.is_finite()) {
        return Err(PetirError::Domain);
    }

    let mut esol = ModNewton1::init(a, h, &dfdy)?;
    let yz = esol.solve(f, a, c, t, h, y0, &errlev)?;

    let mut fyz = vec![0.0_f64; dim];
    f(t + c * h, &yz, &mut fyz);

    // rksubs: y = 0; y += b fY; y *= h; y += y0.
    let mut y = vec![0.0_f64; dim];
    for (yi, &fy) in y.iter_mut().zip(fyz.iter()) {
        *yi = 0.0;
        *yi += b * fy;
    }
    for (yi, &y0i) in y.iter_mut().zip(y0.iter()) {
        *yi *= h;
        *yi += y0i;
    }
    if y.iter().any(|v| !v.is_finite()) {
        return Err(PetirError::Domain);
    }
    Ok(y)
}

/// One backward (implicit) Euler step: `y_new = y + h f(t + h, y_new)`.
///
/// The single step of GSL's `rk1imp_apply` (`rk1imp.c:291-326`): Newton
/// ([`modnewton1`](self) port) for the stage `Y`, then `y_new = y + h f(t + h,
/// Y)`. `jac(t, y, J)` writes `df/dy` at the start of the step into the
/// `n x n` matrix `J` (analytic, or [`fd_jacobian`]).
///
/// # Errors
///
/// - [`PetirError::Invalid`] for an empty state, `h` zero or non-finite, or a
///   non-positive error level (`eps_abs = eps_rel = 0` with `y_i = 0`).
/// - [`PetirError::Singular`] if `I - h J` is singular.
/// - [`PetirError::Diverged`] / [`PetirError::MaxIterations`] if Newton
///   diverges or does not converge in 7 iterations: take a smaller step.
/// - [`PetirError::Domain`] for a non-finite `f`, `J` or result.
///
/// # Example
///
/// ```
/// use petir::linalg::Matrix;
/// use petir::ode::implicit::{backward_euler_step, ImplicitTolerance};
/// // y' = -1000 (y - 1), stiff; one step of h = 0.1 from y = 0.
/// let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = -1000.0 * (y[0] - 1.0);
/// let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, -1000.0);
/// let tol = ImplicitTolerance { eps_abs: 1e-12, eps_rel: 1e-12 };
/// let y = backward_euler_step(&f, &j, 0.0, &[0.0], 0.1, &tol).unwrap();
/// // Exact backward Euler: y1 = 100/101.
/// assert!((y[0] - 100.0 / 101.0).abs() < 1e-10);
/// ```
pub fn backward_euler_step<F, J>(
    f: &F,
    jac: &J,
    t: f64,
    y: &[f64],
    h: f64,
    tol: &ImplicitTolerance,
) -> Result<Vec<f64>>
where
    F: Fn(f64, &[f64], &mut [f64]),
    J: Fn(f64, &[f64], &mut Matrix),
{
    one_stage_step(f, jac, t, h, 1.0, y, y, tol)
}

/// One variable-step BDF2 step from `y` (at `t`) and `y_old` (at
/// `t - h_old`) to `t + h`.
///
/// OpenFOAM's `backwardDdtScheme` coefficients (`backwardDdtScheme.C:676-678`)
/// with `dt = h`, `dt0 = h_old`:
/// `(coefft*y_new - coefft0*y + coefft00*y_old)/h = f(t + h, y_new)`, solved
/// by the [`modnewton1`](self) port as `y_new = ybar + h (1/coefft) f(t + h,
/// y_new)`, `ybar = (coefft0*y - coefft00*y_old)/coefft`. The error levels and
/// the Jacobian are taken at `y`.
///
/// Zero-stable for step ratios `h/h_old < 1 + sqrt(2)`; the caller's grid
/// must respect that.
///
/// # Errors
///
/// As [`backward_euler_step`], plus [`PetirError::Invalid`] for a non-positive
/// or non-finite `h_old`, a sign change between `h` and `h_old`, or
/// `y_old.len() != y.len()`.
#[allow(clippy::too_many_arguments)]
pub fn bdf2_step<F, J>(
    f: &F,
    jac: &J,
    t: f64,
    y: &[f64],
    y_old: &[f64],
    h: f64,
    h_old: f64,
    tol: &ImplicitTolerance,
) -> Result<Vec<f64>>
where
    F: Fn(f64, &[f64], &mut [f64]),
    J: Fn(f64, &[f64], &mut Matrix),
{
    if y_old.len() != y.len() || !h_old.is_finite() || h_old == 0.0 || (h > 0.0) != (h_old > 0.0)
    {
        return Err(PetirError::Invalid);
    }
    let delta_t = h;
    let delta_t0 = h_old;
    let coefft = 1.0 + delta_t / (delta_t + delta_t0);
    let coefft00 = delta_t * delta_t / (delta_t0 * (delta_t + delta_t0));
    let coefft0 = coefft + coefft00;

    let ybar: Vec<f64> = y
        .iter()
        .zip(y_old.iter())
        .map(|(&y0, &y00)| (coefft0 * y0 - coefft00 * y00) / coefft)
        .collect();

    one_stage_step(f, jac, t, h, 1.0 / coefft, &ybar, y, tol)
}

/// Check a time grid: at least two points, all finite, strictly monotone.
fn check_grid(times: &[f64]) -> Result<()> {
    if times.len() < 2 || times.iter().any(|t| !t.is_finite()) {
        return Err(PetirError::Invalid);
    }
    let increasing = times.windows(2).all(|w| matches!(w, [a, b] if b > a));
    let decreasing = times.windows(2).all(|w| matches!(w, [a, b] if b < a));
    if !(increasing || decreasing) {
        return Err(PetirError::Invalid);
    }
    Ok(())
}

/// March backward Euler over the grid `times` (`times[0]` is the initial
/// time, `y0` the state there). Returns every grid point.
///
/// # Errors
///
/// [`PetirError::Invalid`] for fewer than two grid points, a non-finite or
/// non-monotone grid, or an empty `y0`; otherwise as
/// [`backward_euler_step`], at the first step that fails.
///
/// # Example
///
/// ```
/// use petir::linalg::Matrix;
/// use petir::ode::implicit::{solve_backward_euler, ImplicitTolerance};
/// let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = -y[0];
/// let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, -1.0);
/// let grid: Vec<f64> = (0..=100).map(|k| k as f64 * 0.01).collect();
/// let tol = ImplicitTolerance { eps_abs: 1e-14, eps_rel: 1e-12 };
/// let sol = solve_backward_euler(&f, &j, &grid, &[1.0], &tol).unwrap();
/// let y1 = sol.final_state().unwrap()[0];
/// assert!((y1 - (-1.0_f64).exp()).abs() < 2e-3); // first order
/// ```
pub fn solve_backward_euler<F, J>(
    f: &F,
    jac: &J,
    times: &[f64],
    y0: &[f64],
    tol: &ImplicitTolerance,
) -> Result<OdeSolution>
where
    F: Fn(f64, &[f64], &mut [f64]),
    J: Fn(f64, &[f64], &mut Matrix),
{
    check_grid(times)?;
    if y0.is_empty() {
        return Err(PetirError::Invalid);
    }
    let mut y = y0.to_vec();
    let mut points = Vec::with_capacity(times.len());
    for w in times.windows(2) {
        let [t, t_next] = w else {
            return Err(PetirError::Invalid);
        };
        points.push(OdePoint { t: *t, y: y.clone() });
        y = backward_euler_step(f, jac, *t, &y, t_next - t, tol)?;
    }
    if let Some(&t_end) = times.last() {
        points.push(OdePoint { t: t_end, y });
    }
    let accepted = points.len().saturating_sub(1);
    Ok(OdeSolution {
        points,
        accepted,
        rejected: 0,
    })
}

/// March BDF2 over the grid `times`, taking the first step with backward
/// Euler and every later one with the variable-step [`bdf2_step`].
///
/// Keep consecutive step ratios below `1 + sqrt(2)` (BDF2's variable-step
/// zero-stability bound); the grid is not checked against it.
///
/// # Errors
///
/// As [`solve_backward_euler`].
///
/// # Example
///
/// ```
/// use petir::linalg::Matrix;
/// use petir::ode::implicit::{solve_bdf2, ImplicitTolerance};
/// let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = -y[0];
/// let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, -1.0);
/// let grid: Vec<f64> = (0..=100).map(|k| k as f64 * 0.01).collect();
/// let tol = ImplicitTolerance { eps_abs: 1e-14, eps_rel: 1e-12 };
/// let sol = solve_bdf2(&f, &j, &grid, &[1.0], &tol).unwrap();
/// let y1 = sol.final_state().unwrap()[0];
/// assert!((y1 - (-1.0_f64).exp()).abs() < 2e-5); // second order
/// ```
pub fn solve_bdf2<F, J>(
    f: &F,
    jac: &J,
    times: &[f64],
    y0: &[f64],
    tol: &ImplicitTolerance,
) -> Result<OdeSolution>
where
    F: Fn(f64, &[f64], &mut [f64]),
    J: Fn(f64, &[f64], &mut Matrix),
{
    check_grid(times)?;
    if y0.is_empty() {
        return Err(PetirError::Invalid);
    }
    let mut y = y0.to_vec();
    let mut history: Option<(Vec<f64>, f64)> = None; // (y_old, h_old)
    let mut points = Vec::with_capacity(times.len());
    for w in times.windows(2) {
        let [t, t_next] = w else {
            return Err(PetirError::Invalid);
        };
        let h = t_next - t;
        points.push(OdePoint { t: *t, y: y.clone() });
        let y_new = match &history {
            None => backward_euler_step(f, jac, *t, &y, h, tol)?,
            Some((y_old, h_old)) => bdf2_step(f, jac, *t, &y, y_old, h, *h_old, tol)?,
        };
        history = Some((core::mem::replace(&mut y, y_new), h));
    }
    if let Some(&t_end) = times.last() {
        points.push(OdePoint { t: t_end, y });
    }
    let accepted = points.len().saturating_sub(1);
    Ok(OdeSolution {
        points,
        accepted,
        rejected: 0,
    })
}

/// Forward-difference `df/dy` at `(t, y)`, written into the `n x n` `jac`:
/// GSL's `gsl_multiroot_fdjacobian` ([`crate::multiroots::fdjacobian`]) on
/// `y -> f(t, y)` with `epsrel = GSL_SQRT_DBL_EPSILON`.
///
/// Its signature matches the `jac` argument of the steppers once `f` is
/// captured: `let jac = |t, y: &[f64], m: &mut Matrix| fd_jacobian(&f, t, y,
/// m);`. A column that comes out zero (a component `f` does not depend on) is
/// not an error here; see [`crate::multiroots::FdJacobianReport`]. Any other
/// failure (a non-finite perturbed evaluation, mismatched shapes) fills `jac`
/// with NaN, which the stepper then reports as [`PetirError::Domain`].
pub fn fd_jacobian<F>(f: &F, t: f64, y: &[f64], jac: &mut Matrix)
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let mut fx = vec![0.0_f64; y.len()];
    f(t, y, &mut fx);
    let g = |x: &[f64], out: &mut [f64]| f(t, x, out);
    if fdjacobian(g, y, &fx, GSL_SQRT_DBL_EPSILON, jac).is_err() {
        for i in 0..jac.rows() {
            for j in 0..jac.cols() {
                jac.set(i, j, f64::NAN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tol() -> ImplicitTolerance {
        ImplicitTolerance {
            eps_abs: 1e-14,
            eps_rel: 1e-12,
        }
    }

    #[test]
    fn backward_euler_on_a_linear_scalar_is_the_closed_form() {
        // y' = lambda y: y1 = y0 / (1 - h lambda).
        let lam = -50.0;
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = lam * y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, lam);
        let y = backward_euler_step(&f, &j, 0.0, &[2.0], 0.1, &tol()).unwrap();
        assert!((y[0] - 2.0 / (1.0 + 5.0)).abs() < 1e-13);
    }

    #[test]
    fn bdf2_on_a_linear_scalar_is_the_closed_form() {
        // Uniform steps: (3 y2 - 4 y1 + y0)/(2h) = lambda y2.
        let lam = -3.0;
        let h = 0.2;
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = lam * y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, lam);
        let (y0, y1) = (1.0, 0.7);
        let y2 = bdf2_step(&f, &j, h, &[y1], &[y0], h, h, &tol()).unwrap();
        let exact = (4.0 * y1 - y0) / (3.0 - 2.0 * h * lam);
        assert!((y2[0] - exact).abs() < 1e-13, "{} vs {exact}", y2[0]);
    }

    #[test]
    fn fd_jacobian_matches_the_analytic_one() {
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| {
            dy[0] = -y[0] * y[1];
            dy[1] = y[0] * y[0];
        };
        let mut m = Matrix::zeros(2, 2).unwrap();
        fd_jacobian(&f, 0.0, &[2.0, 3.0], &mut m);
        assert!((m.get(0, 0) + 3.0).abs() < 1e-6);
        assert!((m.get(0, 1) + 2.0).abs() < 1e-6);
        assert!((m.get(1, 0) - 4.0).abs() < 1e-6);
        assert!(m.get(1, 1).abs() < 1e-6);
    }

    #[test]
    fn non_finite_jacobian_is_a_domain_error() {
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = -y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, f64::NAN);
        assert_eq!(
            backward_euler_step(&f, &j, 0.0, &[1.0], 0.1, &tol()),
            Err(PetirError::Domain)
        );
    }

    #[test]
    fn newton_divergence_is_reported_not_hidden() {
        // A Jacobian of the wrong sign makes the modified Newton iteration
        // matrix 1 + 100 h instead of 1 - 100 h: the iteration diverges.
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = 100.0 * y[0] * y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, -100.0);
        let r = backward_euler_step(&f, &j, 0.0, &[1.0], 0.5, &tol());
        assert!(
            matches!(r, Err(PetirError::Diverged) | Err(PetirError::MaxIterations)),
            "{r:?}"
        );
    }

    #[test]
    fn bad_grids_and_tolerances_are_rejected() {
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = -y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, -1.0);
        assert_eq!(
            solve_bdf2(&f, &j, &[0.0], &[1.0], &tol()),
            Err(PetirError::Invalid)
        );
        assert_eq!(
            solve_backward_euler(&f, &j, &[0.0, 1.0, 0.5], &[1.0], &tol()),
            Err(PetirError::Invalid)
        );
        let zero = ImplicitTolerance {
            eps_abs: 0.0,
            eps_rel: 0.0,
        };
        assert_eq!(
            backward_euler_step(&f, &j, 0.0, &[0.0], 0.1, &zero),
            Err(PetirError::Invalid)
        );
        assert_eq!(
            bdf2_step(&f, &j, 0.0, &[1.0], &[1.0], 0.1, -0.1, &tol()),
            Err(PetirError::Invalid)
        );
    }

    #[test]
    fn decreasing_grid_integrates_backwards() {
        // y' = y, integrate from t = 1 back to 0 starting at e.
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = y[0];
        let j = |_t: f64, _y: &[f64], m: &mut Matrix| m.set(0, 0, 1.0);
        let grid: Vec<f64> = (0..=200).map(|k| 1.0 - k as f64 / 200.0).collect();
        let sol = solve_bdf2(&f, &j, &grid, &[core::f64::consts::E], &tol()).unwrap();
        assert!((sol.final_state().unwrap()[0] - 1.0).abs() < 1e-4);
        assert_eq!(sol.accepted, 200);
    }
}
